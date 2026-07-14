use axum::extract::Query;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PER_PAGE: i64 = 15;
pub const MAX_PER_PAGE: i64 = 50;

#[derive(Debug, Clone)]
pub struct PaginationQuery {
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PaginationQuery {
    pub fn parse(params: Query<PaginationParams>) -> Self {
        let page = params.page.unwrap_or(1).max(1);
        let per_page = params
            .per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE);
        Self { page, per_page }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> i64 {
        self.per_page
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginationMeta {
    pub current_page: i64,
    pub last_page: i64,
    pub per_page: i64,
    pub total: i64,
}

impl PaginationMeta {
    pub fn new(page: i64, per_page: i64, total: i64) -> Self {
        let last_page = if total == 0 {
            1
        } else {
            (total + per_page - 1) / per_page
        };
        Self {
            current_page: page,
            last_page,
            per_page,
            total,
        }
    }

    pub fn from_query(query: &PaginationQuery, total: i64) -> Self {
        Self::new(query.page, query.per_page, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;

    #[test]
    fn test_default_pagination() {
        let params = Query(PaginationParams {
            page: None,
            per_page: None,
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.page, 1);
        assert_eq!(pq.per_page, 15);
    }

    #[test]
    fn test_custom_pagination() {
        let params = Query(PaginationParams {
            page: Some(3),
            per_page: Some(20),
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.page, 3);
        assert_eq!(pq.per_page, 20);
    }

    #[test]
    fn test_per_page_capped_at_max() {
        let params = Query(PaginationParams {
            page: Some(1),
            per_page: Some(100),
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.per_page, 50);
    }

    #[test]
    fn test_page_minimum_is_one() {
        let params = Query(PaginationParams {
            page: Some(0),
            per_page: Some(10),
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.page, 1);
    }

    #[test]
    fn test_negative_page_becomes_one() {
        let params = Query(PaginationParams {
            page: Some(-5),
            per_page: None,
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.page, 1);
    }

    #[test]
    fn test_zero_per_page_becomes_one() {
        let params = Query(PaginationParams {
            page: None,
            per_page: Some(0),
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.per_page, 1);
    }

    #[test]
    fn test_negative_per_page_becomes_one() {
        let params = Query(PaginationParams {
            page: None,
            per_page: Some(-10),
        });
        let pq = PaginationQuery::parse(params);
        assert_eq!(pq.per_page, 1);
    }

    #[test]
    fn test_offset_calculation() {
        let pq = PaginationQuery {
            page: 3,
            per_page: 15,
        };
        assert_eq!(pq.offset(), 30);
    }

    #[test]
    fn test_offset_first_page() {
        let pq = PaginationQuery {
            page: 1,
            per_page: 15,
        };
        assert_eq!(pq.offset(), 0);
    }

    #[test]
    fn test_limit_equals_per_page() {
        let pq = PaginationQuery {
            page: 1,
            per_page: 25,
        };
        assert_eq!(pq.limit(), 25);
    }

    #[test]
    fn test_meta_basic() {
        let meta = PaginationMeta::new(1, 15, 100);
        assert_eq!(meta.current_page, 1);
        assert_eq!(meta.last_page, 7);
        assert_eq!(meta.per_page, 15);
        assert_eq!(meta.total, 100);
    }

    #[test]
    fn test_meta_exact_division() {
        let meta = PaginationMeta::new(1, 15, 30);
        assert_eq!(meta.last_page, 2);
    }

    #[test]
    fn test_meta_zero_total() {
        let meta = PaginationMeta::new(1, 15, 0);
        assert_eq!(meta.last_page, 1);
        assert_eq!(meta.total, 0);
    }

    #[test]
    fn test_meta_single_item() {
        let meta = PaginationMeta::new(1, 15, 1);
        assert_eq!(meta.last_page, 1);
    }

    #[test]
    fn test_meta_from_query() {
        let query = PaginationQuery {
            page: 2,
            per_page: 10,
        };
        let meta = PaginationMeta::from_query(&query, 42);
        assert_eq!(meta.current_page, 2);
        assert_eq!(meta.last_page, 5);
        assert_eq!(meta.per_page, 10);
        assert_eq!(meta.total, 42);
    }

    #[test]
    fn test_meta_last_page_with_remainder() {
        let meta = PaginationMeta::new(1, 15, 31);
        assert_eq!(meta.last_page, 3);
    }
}
