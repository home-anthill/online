use rocket::http::Status;
use rocket::request::Request;
use tracing::error;

use crate::errors::api_error::ApiError;

#[rocket::catch(400)]
pub fn bad_request(_: &Request) -> ApiError {
    error!(target: "app", "catcher 400 - bad_request");
    ApiError { code: Status::BadRequest.code, message: "Bad request".into() }
}

#[rocket::catch(404)]
pub fn not_found(_: &Request) -> ApiError {
    error!(target: "app", "catcher 404 - not_found");
    ApiError { code: Status::NotFound.code, message: "Not found".into() }
}

#[rocket::catch(500)]
pub fn internal_server_error(_: &Request) -> ApiError {
    error!(target: "app", "catcher 500 - internal_server_error");
    ApiError { code: Status::InternalServerError.code, message: "Internal server error".into() }
}

#[rocket::catch(503)]
pub fn service_unavailable(_: &Request) -> ApiError {
    error!(target: "app", "catcher 503 - service_unavailable");
    ApiError { code: Status::ServiceUnavailable.code, message: "Service Unavailable".into() }
}
