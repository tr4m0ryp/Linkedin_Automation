//! Unit tests for the CSV manager.

use super::*;
use std::io::Write;

fn write_temp_csv(content: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = format!("linkedin_csv_test_{}.csv", uuid::Uuid::new_v4());
    path.push(unique);
    let mut f = std::fs::File::create(&path).expect("create temp csv");
    f.write_all(content.as_bytes()).expect("write temp csv");
    path
}

#[test]
fn legacy_two_column_csv_is_accepted() {
    let path = write_temp_csv(
        "linkedin_url,Is_Sent\n\
         https://www.linkedin.com/in/a/,\n\
         https://www.linkedin.com/in/b/,1\n\
         https://www.linkedin.com/in/c/,\n",
    );
    let mgr = CsvManager::new(&path);
    let (total, unsent) = mgr.counts().expect("counts");
    assert_eq!(total, 3);
    assert_eq!(unsent, 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn write_degree_upgrades_to_four_columns() {
    let path = write_temp_csv(
        "linkedin_url,Is_Sent\n\
         https://www.linkedin.com/in/a/,\n",
    );
    let mgr = CsvManager::new(&path);
    let now = Utc::now();
    mgr.write_degree("https://www.linkedin.com/in/a/", Degree::Second, now)
        .expect("write_degree");

    let rows = mgr.read_all().expect("read_all");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].degree, Degree::Second);
    assert!(rows[0].degree_checked_at.is_some());
    let _ = std::fs::remove_file(path);
}

#[test]
fn read_unsent_needing_recheck_includes_unknown_and_stale_third() {
    let stale = Utc::now() - Duration::days(45);
    let fresh = Utc::now() - Duration::days(5);
    let csv_content = format!(
        "linkedin_url,Is_Sent,degree,degree_checked_at\n\
         https://www.linkedin.com/in/unknown/,,,\n\
         https://www.linkedin.com/in/second/,,2,{fresh}\n\
         https://www.linkedin.com/in/stale-third/,,3,{stale}\n\
         https://www.linkedin.com/in/fresh-third/,,3,{fresh}\n\
         https://www.linkedin.com/in/sent/,1,2,{fresh}\n",
        fresh = fresh.to_rfc3339(),
        stale = stale.to_rfc3339(),
    );
    let path = write_temp_csv(&csv_content);
    let mgr = CsvManager::new(&path);
    let rows = mgr
        .read_unsent_needing_recheck(30)
        .expect("read_unsent_needing_recheck");
    let urls: Vec<String> = rows.into_iter().map(|p| p.linkedin_url).collect();
    assert!(urls.contains(&"https://www.linkedin.com/in/unknown/".to_string()));
    assert!(urls.contains(&"https://www.linkedin.com/in/stale-third/".to_string()));
    assert!(!urls.contains(&"https://www.linkedin.com/in/fresh-third/".to_string()));
    assert!(!urls.contains(&"https://www.linkedin.com/in/second/".to_string()));
    assert!(!urls.contains(&"https://www.linkedin.com/in/sent/".to_string()));
    let _ = std::fs::remove_file(path);
}

#[test]
fn read_unsent_with_degree_filters_correctly() {
    let now = Utc::now().to_rfc3339();
    let csv_content = format!(
        "linkedin_url,Is_Sent,degree,degree_checked_at\n\
         https://www.linkedin.com/in/a/,,2,{now}\n\
         https://www.linkedin.com/in/b/,,3,{now}\n\
         https://www.linkedin.com/in/c/,1,2,{now}\n"
    );
    let path = write_temp_csv(&csv_content);
    let mgr = CsvManager::new(&path);
    let rows = mgr.read_unsent_with_degree(Degree::Second).expect("filter");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].linkedin_url, "https://www.linkedin.com/in/a/");
    let _ = std::fs::remove_file(path);
}
