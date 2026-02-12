//! CSV reader and writer for the linkedin_profiles.csv file.
//!
//! The CSV has two columns: `linkedin_url` and `Is_Sent`.
//! `Is_Sent` is empty for unsent profiles and "1" for sent ones.

use crate::error::{LinkedInError, Result};
use super::types::CsvProfile;
use std::path::{Path, PathBuf};
use tracing::{info, debug};

/// Manages reading and atomic updating of the profile CSV.
pub struct CsvManager {
    path: PathBuf,
}

impl CsvManager {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Read all profiles from the CSV.
    fn read_all(&self) -> Result<Vec<CsvProfile>> {
        let mut reader = csv::Reader::from_path(&self.path).map_err(|e| {
            LinkedInError::CsvError(format!("Failed to open {}: {}", self.path.display(), e))
        })?;

        let mut profiles = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|e| {
                LinkedInError::CsvError(format!("Malformed CSV row: {}", e))
            })?;

            let url = record.get(0).unwrap_or("").trim().to_string();
            let sent_field = record.get(1).unwrap_or("").trim().to_string();
            let is_sent = sent_field == "1";

            if !url.is_empty() {
                profiles.push(CsvProfile {
                    linkedin_url: url,
                    is_sent,
                });
            }
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

    /// Mark a profile URL as sent by rewriting the CSV atomically.
    ///
    /// Reads all rows, sets `Is_Sent=1` for the matching URL, writes to
    /// a temporary file, then renames over the original.
    pub fn mark_sent(&self, url: &str) -> Result<()> {
        let profiles = self.read_all()?;

        let temp_path = self.path.with_extension("csv.tmp");
        {
            let mut writer = csv::Writer::from_path(&temp_path).map_err(|e| {
                LinkedInError::CsvError(format!("Failed to create temp file: {}", e))
            })?;

            writer.write_record(["linkedin_url", "Is_Sent"]).map_err(|e| {
                LinkedInError::CsvError(format!("Failed to write header: {}", e))
            })?;

            for profile in &profiles {
                let sent_value = if profile.is_sent || profile.linkedin_url == url {
                    "1"
                } else {
                    ""
                };
                writer
                    .write_record([&profile.linkedin_url, sent_value])
                    .map_err(|e| {
                        LinkedInError::CsvError(format!("Failed to write row: {}", e))
                    })?;
            }

            writer.flush().map_err(|e| {
                LinkedInError::CsvError(format!("Failed to flush CSV: {}", e))
            })?;
        }

        std::fs::rename(&temp_path, &self.path).map_err(|e| {
            LinkedInError::CsvError(format!("Failed to rename temp file: {}", e))
        })?;

        debug!("Marked as sent: {}", url);
        Ok(())
    }
}
