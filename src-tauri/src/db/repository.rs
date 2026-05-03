use super::models::*;
use super::Database;
use crate::db::models::ListFilesParams;
use std::collections::HashMap;

impl Database {
    pub fn insert_share_link(
        &self,
        share_code: &str,
        receive_code: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.get_conn();
        conn.execute(
            "INSERT INTO share_links (share_code, receive_code, status) VALUES (?1, ?2, 'pending')",
            rusqlite::params![share_code, receive_code],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_share_link_status(
        &self,
        id: i64,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        conn.execute(
            "UPDATE share_links SET status = ?1, error_message = ?2 WHERE id = ?3",
            rusqlite::params![status, error_msg, id],
        )?;
        Ok(())
    }

    pub fn update_share_link_metadata(
        &self,
        id: i64,
        title: &str,
        user_id: &str,
        user_name: &str,
        file_count: i64,
        total_size: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        conn.execute(
            "UPDATE share_links SET title = ?1, share_user_id = ?2, share_user_name = ?3, \
             total_file_count = ?4, total_size = ?5, status = 'completed', \
             last_parsed_at = datetime('now') WHERE id = ?6",
            rusqlite::params![title, user_id, user_name, file_count, total_size, id],
        )?;
        Ok(())
    }

    pub fn get_share_link(&self, id: i64) -> Result<Option<ShareLink>, rusqlite::Error> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, share_code, receive_code, title, share_user_id, share_user_name, \
             total_file_count, total_size, status, error_message, last_parsed_at, added_at \
             FROM share_links WHERE id = ?1",
        )?;
        let result = stmt.query_row(rusqlite::params![id], |row| {
            Ok(ShareLink {
                id: row.get(0)?,
                share_code: row.get(1)?,
                receive_code: row.get(2)?,
                title: row.get(3)?,
                share_user_id: row.get(4)?,
                share_user_name: row.get(5)?,
                total_file_count: row.get(6)?,
                total_size: row.get(7)?,
                status: row.get(8)?,
                error_message: row.get(9)?,
                last_parsed_at: row.get(10)?,
                added_at: row.get(11)?,
            })
        });
        match result {
            Ok(sl) => Ok(Some(sl)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_share_links(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ShareLink>, i64), rusqlite::Error> {
        let conn = self.get_conn();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM share_links", [], |row| row.get(0))?;

        let offset = (page.saturating_sub(1)) * page_size;
        let mut stmt = conn.prepare(
            "SELECT id, share_code, receive_code, title, share_user_id, share_user_name, \
             total_file_count, total_size, status, error_message, last_parsed_at, added_at \
             FROM share_links ORDER BY added_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let items = stmt
            .query_map(rusqlite::params![page_size, offset], |row| {
                Ok(ShareLink {
                    id: row.get(0)?,
                    share_code: row.get(1)?,
                    receive_code: row.get(2)?,
                    title: row.get(3)?,
                    share_user_id: row.get(4)?,
                    share_user_name: row.get(5)?,
                    total_file_count: row.get(6)?,
                    total_size: row.get(7)?,
                    status: row.get(8)?,
                    error_message: row.get(9)?,
                    last_parsed_at: row.get(10)?,
                    added_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok((items, total))
    }

    pub fn delete_share_link(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        conn.execute("DELETE FROM share_links WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn delete_files_by_share_link(&self, share_link_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        conn.execute(
            "DELETE FROM files WHERE share_link_id = ?1",
            rusqlite::params![share_link_id],
        )?;
        Ok(())
    }

    pub fn insert_files_batch(
        &self,
        files: &[(i64, &str, &str, &str, i64, &str, bool, &str, &str, i32, &str)],
    ) -> Result<(), rusqlite::Error> {
        let mut conn = self.get_conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO files (share_link_id, file_id, parent_id, name, size, sha1, \
                 is_dir, file_type, full_path, depth, thumbnail_url) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for (share_link_id, file_id, parent_id, name, size, sha1, is_dir, file_type, full_path, depth, thumbnail_url) in files {
                stmt.execute(rusqlite::params![
                    share_link_id, file_id, parent_id, name, size, sha1,
                    if *is_dir { 1 } else { 0 }, file_type, full_path, depth, thumbnail_url
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn search_files(&self, params: &SearchParams) -> Result<SearchResult, rusqlite::Error> {
        let conn = self.get_conn();
        let page = params.page.unwrap_or(1).max(1);
        let page_size = params.page_size.unwrap_or(10).min(200);
        let offset = (page - 1) * page_size;

        let mut where_clauses = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        let use_fts = params.query.as_ref().map_or(false, |q| !q.is_empty());

        if use_fts {
            if let Some(ref q) = params.query {
                // Hybrid: FTS5 prefix match + LIKE substring fallback.
                // FTS5 with "keyword"* handles prefix/whole-word matching;
                // LIKE '%keyword%' catches substrings that tokenization misses.
                let fts_query = format!("\"{}\"*", q.replace('\"', ""));
                let like_pattern = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));

                where_clauses.push(format!(
                    "(f.id IN (SELECT rowid FROM files_fts WHERE files_fts MATCH ?{0}) OR f.name LIKE ?{1} ESCAPE '\\')",
                    param_idx, param_idx + 1
                ));
                param_values.push(Box::new(fts_query));
                param_values.push(Box::new(like_pattern));
                param_idx += 2;
            }
        }

        if let Some(ref ft) = params.file_type {
            where_clauses.push(format!("f.file_type = ?{}", param_idx));
            param_values.push(Box::new(ft.clone()));
            param_idx += 1;
        }

        if let Some(min) = params.size_min {
            where_clauses.push(format!("f.size >= ?{}", param_idx));
            param_values.push(Box::new(min));
            param_idx += 1;
        }

        if let Some(max) = params.size_max {
            where_clauses.push(format!("f.size <= ?{}", param_idx));
            param_values.push(Box::new(max));
            param_idx += 1;
        }

        if let Some(slid) = params.share_link_id {
            where_clauses.push(format!("f.share_link_id = ?{}", param_idx));
            param_values.push(Box::new(slid));
            param_idx += 1;
        }

        if let Some(is_dir) = params.is_dir {
            where_clauses.push(format!("f.is_dir = ?{}", param_idx));
            param_values.push(Box::new(if is_dir { 1 } else { 0 }));
            param_idx += 1;
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sort_by = match params.sort_by.as_deref() {
            Some("size") => "f.size",
            Some("name") => "f.name",
            Some("date") => "f.created_at",
            Some("relevance") if use_fts => "rank",
            _ => "f.name",
        };
        let sort_order = match params.sort_order.as_deref() {
            Some("asc") => "ASC",
            _ => "DESC",
        };

        let count_sql = format!("SELECT COUNT(*) FROM files f {}", where_str);
        let total_count: i64 = conn.query_row(
            &count_sql,
            param_values.iter().map(|p| p.as_ref()).collect::<Vec<_>>().as_slice(),
            |row| row.get(0),
        )?;

        let query_sql = format!(
            "SELECT f.id, f.share_link_id, f.file_id, f.parent_id, f.name, f.size, f.sha1, \
             f.is_dir, f.file_type, f.full_path, f.depth, f.thumbnail_url \
             FROM files f {} ORDER BY {} {} LIMIT ?{} OFFSET ?{}",
            where_str, sort_by, sort_order, param_idx, param_idx + 1
        );

        let limit_i64 = page_size as i64;
        let offset_i64 = offset as i64;
        param_values.push(Box::new(limit_i64));
        param_values.push(Box::new(offset_i64));

        let all_params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query_sql)?;
        let items = stmt
            .query_map(all_params.as_slice(), |row| {
                Ok(FileEntry {
                    id: row.get(0)?,
                    share_link_id: row.get(1)?,
                    file_id: row.get(2)?,
                    parent_id: row.get(3)?,
                    name: row.get(4)?,
                    size: row.get(5)?,
                    sha1: row.get(6)?,
                    is_dir: row.get::<_, i32>(7)? != 0,
                    file_type: row.get(8)?,
                    full_path: row.get(9)?,
                    depth: row.get(10)?,
                    thumbnail_url: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SearchResult {
            items,
            total_count,
            page,
            page_size,
        })
    }

    pub fn get_file_stats(&self) -> Result<AppStats, rusqlite::Error> {
        let conn = self.get_conn();

        let total_share_links: i64 =
            conn.query_row("SELECT COUNT(*) FROM share_links", [], |row| row.get(0))?;

        let total_files: i64 =
            conn.query_row("SELECT COUNT(*) FROM files WHERE is_dir = 0", [], |row| {
                row.get(0)
            })?;

        let total_size: i64 =
            conn.query_row("SELECT COALESCE(SUM(size), 0) FROM files WHERE is_dir = 0", [], |row| {
                row.get(0)
            })?;

        let parsing_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM share_links WHERE status = 'parsing'",
            [],
            |row| row.get(0),
        )?;

        let completed_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM share_links WHERE status = 'completed'",
            [],
            |row| row.get(0),
        )?;

        let error_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM share_links WHERE status = 'error'",
            [],
            |row| row.get(0),
        )?;

        let mut files_by_type = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT file_type, COUNT(*) as cnt FROM files WHERE is_dir = 0 GROUP BY file_type",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (ft, cnt) = row?;
                files_by_type.insert(ft, cnt);
            }
        }

        Ok(AppStats {
            total_share_links,
            total_files,
            total_size,
            files_by_type,
            parsing_count,
            completed_count,
            error_count,
        })
    }

    pub fn list_files(
        &self,
        params: &ListFilesParams,
    ) -> Result<(Vec<FileEntry>, i64), rusqlite::Error> {
        let conn = self.get_conn();
        let page = params.page.unwrap_or(1).max(1);
        let page_size = params.page_size.unwrap_or(10).min(200);
        let offset = (page - 1) * page_size;

        let mut where_clauses = vec!["f.is_dir = 0".to_string()];
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref ft) = params.file_type {
            where_clauses.push(format!("f.file_type = ?{}", idx));
            param_values.push(Box::new(ft.clone()));
            idx += 1;
        }

        if let Some(ref kw) = params.keyword {
            if !kw.is_empty() {
                where_clauses.push(format!("f.name LIKE ?{}", idx));
                param_values.push(Box::new(format!("%{}%", kw)));
                idx += 1;
            }
        }

        if let Some(slid) = params.share_link_id {
            where_clauses.push(format!("f.share_link_id = ?{}", idx));
            param_values.push(Box::new(slid));
            idx += 1;
        }

        let where_str = where_clauses.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM files f WHERE {}", where_str);
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let total: i64 = conn.query_row(&count_sql, params_ref.as_slice(), |row| row.get(0))?;

        let limit_i64 = page_size as i64;
        let offset_i64 = offset as i64;
        param_values.push(Box::new(limit_i64));
        param_values.push(Box::new(offset_i64));

        let query_sql = format!(
            "SELECT f.id, f.share_link_id, f.file_id, f.parent_id, f.name, f.size, f.sha1, \
             f.is_dir, f.file_type, f.full_path, f.depth, f.thumbnail_url \
             FROM files f WHERE {} ORDER BY f.size DESC LIMIT ?{} OFFSET ?{}",
            where_str, idx, idx + 1
        );

        let all_params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query_sql)?;
        let items = stmt
            .query_map(all_params.as_slice(), |row| {
                Ok(FileEntry {
                    id: row.get(0)?,
                    share_link_id: row.get(1)?,
                    file_id: row.get(2)?,
                    parent_id: row.get(3)?,
                    name: row.get(4)?,
                    size: row.get(5)?,
                    sha1: row.get(6)?,
                    is_dir: row.get::<_, i32>(7)? != 0,
                    file_type: row.get(8)?,
                    full_path: row.get(9)?,
                    depth: row.get(10)?,
                    thumbnail_url: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok((items, total))
    }

    /// List files/dirs inside a specific directory of a share link.
    /// parent_id = "" or "0" means root of the share.
    pub fn list_files_in_dir(
        &self,
        share_link_id: i64,
        parent_id: &str,
        file_type: Option<&str>,
        keyword: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<FileEntry>, i64), rusqlite::Error> {
        let conn = self.get_conn();
        let page = page.max(1);
        let page_size = page_size.min(200);
        let offset = ((page - 1) * page_size) as i64;

        // Normalize: empty string means root directory ("0")
        let effective_parent_id = if parent_id.is_empty() { "0" } else { parent_id };

        let mut where_clauses = vec![
            "f.share_link_id = ?1".to_string(),
            "f.parent_id = ?2".to_string(),
        ];
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(share_link_id));
        param_values.push(Box::new(effective_parent_id.to_string()));
        let mut idx = 3;

        if let Some(ft) = file_type {
            where_clauses.push(format!("f.file_type = ?{}", idx));
            param_values.push(Box::new(ft.to_string()));
            idx += 1;
        }

        if let Some(kw) = keyword {
            if !kw.is_empty() {
                where_clauses.push(format!("f.name LIKE ?{}", idx));
                param_values.push(Box::new(format!("%{}%", kw)));
                idx += 1;
            }
        }

        let where_str = where_clauses.join(" AND ");
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let count_sql = format!("SELECT COUNT(*) FROM files f WHERE {}", where_str);
        let total: i64 = conn.query_row(&count_sql, params_ref.as_slice(), |row| row.get(0))?;

        let limit_i64 = page_size as i64;
        param_values.push(Box::new(limit_i64));
        param_values.push(Box::new(offset));

        // Dirs first, then files, sorted by name
        let query_sql = format!(
            "SELECT f.id, f.share_link_id, f.file_id, f.parent_id, f.name, f.size, f.sha1, \
             f.is_dir, f.file_type, f.full_path, f.depth, f.thumbnail_url \
             FROM files f WHERE {} ORDER BY f.is_dir DESC, f.name ASC LIMIT ?{} OFFSET ?{}",
            where_str, idx, idx + 1
        );

        let all_params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query_sql)?;
        let items = stmt
            .query_map(all_params.as_slice(), |row| {
                Ok(FileEntry {
                    id: row.get(0)?,
                    share_link_id: row.get(1)?,
                    file_id: row.get(2)?,
                    parent_id: row.get(3)?,
                    name: row.get(4)?,
                    size: row.get(5)?,
                    sha1: row.get(6)?,
                    is_dir: row.get::<_, i32>(7)? != 0,
                    file_type: row.get(8)?,
                    full_path: row.get(9)?,
                    depth: row.get(10)?,
                    thumbnail_url: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok((items, total))
    }

    /// Recursively get all non-dir file_ids under a given parent_id for a share link.
    pub fn get_all_file_ids_in_dir(
        &self,
        share_link_id: i64,
        parent_id: &str,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.get_conn();
        let mut all_ids: Vec<String> = Vec::new();
        let mut stack: Vec<String> = vec![parent_id.to_string()];

        while let Some(pid) = stack.pop() {
            let mut stmt = conn.prepare(
                "SELECT file_id, is_dir FROM files WHERE share_link_id = ?1 AND parent_id = ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![share_link_id, pid], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })?;
            for row in rows {
                let (fid, is_dir) = row?;
                if is_dir != 0 {
                    stack.push(fid);
                } else {
                    all_ids.push(fid);
                }
            }
        }

        Ok(all_ids)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.get_conn();
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }
}
