//! Core HTTP client for the LinkedIn Voyager API. Decoy helpers in `decoys`.

mod decoys;

use super::session;
use super::types::{ConnectionState, InvitationResponse, ProfileData, SessionConfig};
use crate::error::{LinkedInError, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use tracing::{debug, warn};

/// Authenticated HTTP client for LinkedIn's internal Voyager API.
pub struct LinkedInClient {
    pub(super) client: reqwest::Client,
    csrf_token: String,
    cookie_file: String,
}

impl LinkedInClient {
    /// Build a new client from a `SessionConfig`.
    pub fn new(config: &SessionConfig) -> Result<Self> {
        let jar = session::load_cookies(&config.cookie_file)?;
        let client = reqwest::Client::builder()
            .cookie_provider(jar)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| LinkedInError::ApiError(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            csrf_token: config.csrf_token.clone(),
            cookie_file: config.cookie_file.clone(),
        })
    }

    pub fn cookie_file(&self) -> &str {
        &self.cookie_file
    }

    pub(crate) fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&format!("ajax:{}", self.csrf_token)) {
            headers.insert("csrf-token", v);
        }
        headers.insert(
            "x-restli-protocol-version",
            HeaderValue::from_static("2.0.0"),
        );
        headers.insert("x-li-lang", HeaderValue::from_static("en_US"));
        let track = r#"{"clientVersion":"1.13.42372","mpVersion":"1.13.42372","osName":"web","timezoneOffset":0,"deviceFormFactor":"DESKTOP","mpName":"voyager-web","displayDensity":1,"displayWidth":1920,"displayHeight":1080}"#;
        if let Ok(v) = HeaderValue::from_str(track) {
            headers.insert("x-li-track", v);
        }
        headers
    }

    /// Resolve a LinkedIn profile URL to structured profile data.
    pub async fn resolve_profile(&self, profile_url: &str) -> Result<ProfileData> {
        let public_id = extract_public_id(profile_url)?;

        let api_url = format!(
            "https://www.linkedin.com/voyager/api/identity/dash/profiles\
             ?q=memberIdentity&memberIdentity={}\
             &decorationId=com.linkedin.voyager.dash.deco.identity.profile.WebTopCardCore-16",
            public_id
        );

        let resp = self
            .client
            .get(&api_url)
            .headers(self.default_headers())
            .send()
            .await
            .map_err(|e| LinkedInError::ApiError(format!("Profile request failed: {}", e)))?;

        let status = resp.status().as_u16();
        if status == 401 || status == 403 {
            return Err(LinkedInError::SessionExpired);
        }
        if status == 429 {
            return Err(LinkedInError::RateLimitExceeded { retry_after: 60 });
        }
        if !resp.status().is_success() {
            return Err(LinkedInError::ProfileResolutionError(format!(
                "HTTP {} for profile {}",
                status, public_id
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            LinkedInError::ProfileResolutionError(format!("JSON parse error: {}", e))
        })?;

        parse_profile_response(&public_id, &body)
    }

    /// Send a connection invitation to the given profile.
    pub async fn send_invitation(&self, profile: &ProfileData) -> Result<InvitationResponse> {
        let payload = serde_json::json!({
            "invitee": {
                "inviteeUnion": {
                    "memberProfile": profile.profile_urn
                }
            },
            "customMessage": ""
        });

        debug!(
            "Sending invitation to {} ({})",
            profile.public_id, profile.profile_urn
        );

        let url = "https://www.linkedin.com/voyager/api/\
            voyagerRelationshipsDashMemberRelationships\
            ?action=verifyQuotaAndCreateV2\
            &decorationId=com.linkedin.voyager.dash.deco.relationships.\
            InvitationCreationResultWithInvitee-2";

        let resp = self
            .client
            .post(url)
            .headers(self.default_headers())
            .header("accept", "application/vnd.linkedin.normalized+json+2.1")
            .json(&payload)
            .send()
            .await
            .map_err(|e| LinkedInError::ApiError(format!("Invitation request failed: {}", e)))?;

        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();

        if status == 429 {
            return Err(LinkedInError::RateLimitExceeded { retry_after: 60 });
        }
        if status == 401 || status == 403 {
            return Err(LinkedInError::SessionExpired);
        }

        // CANT_RESEND_YET means a pending invitation already exists
        if status == 400 && body.contains("CANT_RESEND_YET") {
            return Ok(InvitationResponse {
                success: false,
                status_code: status,
                body: "ALREADY_PENDING".to_string(),
            });
        }

        let success = (200..300).contains(&status);
        if !success {
            warn!("Invitation API returned {}: {}", status, body);
        }

        Ok(InvitationResponse {
            success,
            status_code: status,
            body,
        })
    }
}

fn extract_public_id(url: &str) -> Result<String> {
    let trimmed = url.trim().trim_end_matches('/');
    if let Some(pos) = trimmed.rfind("/in/") {
        let slug = &trimmed[pos + 4..];
        let slug = slug.split('?').next().unwrap_or(slug);
        if slug.is_empty() {
            return Err(LinkedInError::ProfileResolutionError(
                "Empty public identifier in URL".to_string(),
            ));
        }
        Ok(slug.to_string())
    } else {
        Err(LinkedInError::ProfileResolutionError(format!(
            "Cannot extract public ID from URL: {}",
            url
        )))
    }
}

/// Parse the `/identity/dash/profiles` response into `ProfileData`.
fn parse_profile_response(public_id: &str, body: &serde_json::Value) -> Result<ProfileData> {
    let element = body.pointer("/elements/0").ok_or_else(|| {
        LinkedInError::ProfileResolutionError(format!(
            "No elements in profile response for {}",
            public_id
        ))
    })?;

    let profile_urn = element
        .get("entityUrn")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Extract the member ID from the URN (last colon-separated segment)
    let member_id = profile_urn.rsplit(':').next().unwrap_or("").to_string();

    let first_name = element
        .get("firstName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let last_name = element
        .get("lastName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let connection_state = parse_member_relationship(element);
    let member_distance = parse_member_distance(element);

    if member_id.is_empty() || profile_urn.is_empty() {
        return Err(LinkedInError::ProfileResolutionError(format!(
            "Could not extract member ID for {}",
            public_id,
        )));
    }

    Ok(ProfileData {
        public_id: public_id.to_string(),
        member_id,
        profile_urn,
        first_name,
        last_name,
        connection_state,
        member_distance,
    })
}

/// Parse the `memberRelationship` field from the dash profiles response.
///
/// The structure is:
///   memberRelationship.memberRelationshipUnion.noConnection -> NotConnected
///   memberRelationship.memberRelationshipUnion.connection -> Connected
///   memberRelationship.memberRelationshipUnion.invitation -> Pending (approx.)
fn parse_member_relationship(element: &serde_json::Value) -> ConnectionState {
    let union = element.pointer("/memberRelationship/memberRelationshipUnion");
    match union {
        Some(u) => {
            if u.get("noConnection").is_some() {
                ConnectionState::NotConnected
            } else if u.get("connection").is_some() {
                ConnectionState::Connected
            } else if u.get("invitation").is_some() {
                ConnectionState::Pending
            } else {
                ConnectionState::Unknown
            }
        },
        None => ConnectionState::Unknown,
    }
}

/// Parse the `memberDistance` field from the dash profiles element.
///
/// The WebTopCardCore-16 decoration emits a string value such as
/// `"DISTANCE_2"` or `"OUT_OF_NETWORK"`. The exact key has shifted across
/// deploys, so we probe `value`, then `distance`, then `distanceValue`.
fn parse_member_distance(element: &serde_json::Value) -> Option<i32> {
    let raw = element
        .pointer("/memberDistance/value")
        .or_else(|| element.pointer("/memberDistance/distance"))
        .or_else(|| element.pointer("/memberDistance/distanceValue"))
        .and_then(|v| v.as_str());

    let raw = match raw {
        Some(s) => s,
        None => {
            debug!("memberDistance not present in profile element");
            return None;
        },
    };

    if raw == "OUT_OF_NETWORK" {
        return Some(4);
    }
    if let Some(suffix) = raw.strip_prefix("DISTANCE_") {
        if let Ok(n) = suffix.parse::<i32>() {
            return Some(n);
        }
    }
    debug!(
        raw = raw,
        "memberDistance value did not match known patterns"
    );
    None
}
