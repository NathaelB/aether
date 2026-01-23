use axum::{Json, body::Body, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response<T: Serialize + PartialEq> {
    OK(T),
    Created(T),
    NoContent,
    Accepted(T),
}

impl<T: Serialize + PartialEq> IntoResponse for Response<T> {
    fn into_response(self) -> axum::response::Response {
        match self {
            Response::OK(data) => (StatusCode::OK, Json(data)).into_response(),
            Response::Created(data) => (StatusCode::CREATED, Json(data)).into_response(),
            Response::Accepted(data) => (StatusCode::ACCEPTED, Json(data)).into_response(),
            Response::NoContent => (StatusCode::NO_CONTENT, Body::empty()).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Response;
    use axum::response::IntoResponse;
    use serde::Serialize;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    struct Sample {
        value: &'static str,
    }

    #[test]
    fn response_status_codes() {
        let ok = Response::OK(Sample { value: "ok" }).into_response();
        assert_eq!(ok.status(), axum::http::StatusCode::OK);

        let created = Response::Created(Sample { value: "created" }).into_response();
        assert_eq!(created.status(), axum::http::StatusCode::CREATED);

        let accepted = Response::Accepted(Sample { value: "accepted" }).into_response();
        assert_eq!(accepted.status(), axum::http::StatusCode::ACCEPTED);

        let no_content: axum::response::Response = Response::<Sample>::NoContent.into_response();
        assert_eq!(no_content.status(), axum::http::StatusCode::NO_CONTENT);
    }
}
