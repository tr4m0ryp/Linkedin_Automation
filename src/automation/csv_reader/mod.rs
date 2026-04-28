//! CSV reader and writer for the linkedin_profiles.csv file.
//!
//! Schema (post-Task-001, per D4):
//! `linkedin_url,Is_Sent,degree,degree_checked_at`
//!
//! - `Is_Sent`: empty for unsent, `"1"` for sent.
//! - `degree`: empty (unfetched), `"2"`, or `"3"`.
//! - `degree_checked_at`: empty or RFC3339 timestamp.
//!
//! Backward compatibility: legacy two-column files are accepted via
//! `csv::ReaderBuilder::flexible(true)`. Missing trailing fields are treated
//! as `Unknown`/`None`. The next write upgrades the file to four columns.

use super::types::{CsvProfile, Degree};
use crate::error::{LinkedInError, Result};
use chrono::{DateTime, Duration, Utc};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

#[cfg(test)]
mod tests;

const CSV_HEADERS: [&str; 4] = ["linkedin_url", "Is_Sent", "degree", "degree_checked_at"];

/// Manages reading and atomic updating of the profile CSV.
pub struct CsvManager {
    path: PathBuf,
}

impl CsvManager {
    /// Build a manager for the CSV at the given path. The file is not opened
    /// until a read or write call.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Read all profiles from the CSV. Tolerates legacy 2-column files.
    fn read_all(&self) -> Result<Vec<CsvProfile>> {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_path(&self.path)
            .map_err(|e| {
                LinkedInError::CsvError(format!("Failed to open {}: {}", self.path.display(), e))
            })?;

        let mut profiles = Vec::new();
        for record in reader.records() {
            let record =
                record.map_err(|e| LinkedInError::CsvError(format!("Malformed CSV row: {}", e)))?;

            let url = record.get(0).unwrap_or("").trim().to_string();
            if url.is_empty() {
                continue;
            }

            let is_sent = record.get(1).unwrap_or("").trim() == "1";
            let degree = Degree::from_csv_value(record.get(2).unwrap_or(""));
            let degree_checked_at = parse_optional_rfc3339(record.get(3).unwrap_or(""));

            profiles.push(CsvProfile {
                linkedin_url: url,
                is_sent,
                degree,
                degree_checked_at,
            });
        }

        debug!("Read {} profiles from CSV", profiles.len());
        Ok(profiles)
    }

    /// Return only profiles where `Is_Sent` is not "1".
    pub fn read_unsent(&self) -> Result<Vec<CsvProfile>> {
        let all = self.read_all()?;
        let unsent: Vec<CsvProfile> = all.into_iter().filter(|p| !p.is_sent).collect();
        info!("Found {} unsent profiles", unsent.len());
        Ok(unsent)
    }

    /// Return total and unsent counts for display.
    pub fn counts(&self) -> Result<(usize, usize)> {
        let all = self.read_all()?;
        let total = all.len();
        let unsent = all.iter().filter(|p| !p.is_sent).count();
        Ok((total, unsent))
    }

    /// Return unsent profiles whose cached `degree` matches the given value.
    ///
    /// Used by the ranker (Task 004) to pull only sendable 2nd-degree
    /// candidates.
    pub fn read_unsent_with_degree(&self, degree: Degree) -> Result<Vec<CsvProfile>> {
        let all = self.read_all()?;
        let filtered: Vec<CsvProfile> = all
            .into_iter()
            .filter(|p| !p.is_sent && p.degree == degree)
            .collect();
        debug!(
            count = filtered.len(),
            "Filtered unsent profiles by degree {}", degree
        );
        Ok(filtered)
    }

    /// Return unsent profiles whose `degree` should be (re-)fetched.
    ///
    /// A row qualifies when:
    /// - `is_sent == false`, AND
    /// - `degree == Unknown`, OR
    /// - `degree == ThirdOrMore` AND (`degree_checked_at` is `None` or older
    ///   than `recheck_days`).
    ///
    /// This is the discovery-pass fetch list (D4 step 1+3).
    pub fn read_unsent_needing_recheck(&self, recheck_days: i64) -> Result<Vec<CsvProfile>> {
        let all = self.read_all()?;
        let now = Utc::now();
        let recheck_threshold = Duration::days(recheck_days.max(0));

        let needs_recheck = |p: &CsvProfile| -> bool {
            if p.is_sent {
                return false;
            }
            match p.degree {
                Degree::Unknown => true,
                Degree::ThirdOrMore => match p.degree_checked_at {
                    None => true,
                    Some(ts) => now.signed_duration_since(ts) >= recheck_threshold,
                },
                Degree::Second => false,
            }
        };

        let filtered: Vec<CsvProfile> = all.into_iter().filter(needs_recheck).collect();
        debug!(
            count = filtered.len(),
            recheck_days, "Selected profiles needing degree recheck"
        );
        Ok(filtered)
    }

    /// Mark a profile URL as sent by rewriting the CSV atomically.
    pub fn mark_sent(&self, url: &str) -> Result<()> {
        let mut profiles = self.read_all()?;
        for profile in profiles.iter_mut() {
            if profile.linkedin_url == url {
                profile.is_sent = true;
            }
        }
        self.write_all(&profiles)?;
        debug!("Marked as sent: {}", url);
        Ok(())
    }

    /// Persist a freshly observed degree for `url`, stamped with `checked_at`.
    ///
    /// Rewrites the CSV atomically. If the URL is not present, the file is
    /// rewritten unchanged.
    pub fn write_degree(&self, url: &str, degree: Degree, checked_at: DateTime<Utc>) -> Result<()> {
        let mut profiles = self.read_all()?;
        let mut matched = false;
        for profile in profiles.iter_mut() {
            if profile.linkedin_url == url {
                profile.degree = degree;
                profile.degree_checked_at = Some(checked_at);
                matched = true;
            }
        }
        if !matched {
            debug!(profile = %url, "write_degree: url not found in CSV");
        }
        self.write_all(&profiles)?;
        Ok(())
    }

    /// Atomically write the full CSV (4 columns) via a `.tmp` rename.
    fn write_all(&self, profiles: &[CsvProfile]) -> Result<()> {
        let temp_path = self.path.with_extension("csv.tmp");
        {
            let mut writer = csv::Writer::from_path(&temp_path).map_err(|e| {
                LinkedInError::CsvError(format!("Failed to create temp file: {}", e))
            })?;

            writer
                .write_record(CSV_HEADERS)
                .map_err(|e| LinkedInError::CsvError(format!("Failed to write header: {}", e)))?;

            for profile in profiles {
                let sent_value = if profile.is_sent { "1" } else { "" };
                let degree_value = profile.degree.to_string();
                let checked_value = profile
                    .degree_checked_at
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default();

                writer
                    .write_record([
                        profile.linkedin_url.as_str(),
                        sent_value,
                        degree_value.as_str(),
                        checked_value.as_str(),
                    ])
                    .map_err(|e| LinkedInError::CsvError(format!("Failed to write row: {}", e)))?;
            }

            writer
                .flush()
                .map_err(|e| LinkedInError::CsvError(format!("Failed to flush CSV: {}", e)))?;
        }

        std::fs::rename(&temp_path, &self.path)
            .map_err(|e| LinkedInError::CsvError(format!("Failed to rename temp file: {}", e)))?;
        Ok(())
    }
}

fn parse_optional_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(trimmed)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
