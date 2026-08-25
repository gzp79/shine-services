use crate::web::responses::{problems, ErrorResponse, Problem, ProblemConfig};
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, Path, Query, Request,
    },
    http::request::Parts,
    Extension, Json, RequestExt, RequestPartsExt,
};
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Cow;
use thiserror::Error as ThisError;
use validator::{Validate, ValidationError, ValidationErrors, ValidationErrorsKind};

/// Drop the `value` param the validator derive injects into every failing rule. It echoes
/// the whole offending field (potentially large user input) back in the error response,
/// which wastes bandwidth and leaks the input without adding diagnostic value.
fn strip_value_params(errors: &mut ValidationErrors) {
    for kind in errors.errors_mut().values_mut() {
        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                for error in field_errors {
                    error.params.remove("value");
                }
            }
            ValidationErrorsKind::Struct(nested) => strip_value_params(nested),
            ValidationErrorsKind::List(items) => {
                for nested in items.values_mut() {
                    strip_value_params(nested);
                }
            }
        }
    }
}

pub trait ValidationErrorEx {
    fn with_message<N>(self, message: N) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>;

    fn with_param<N, T>(self, name: N, val: &T) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>,
        T: Serialize;

    fn into_constraint_error(self, field: &'static str) -> InputError
    where
        Self: Sized;
}

impl ValidationErrorEx for ValidationError {
    fn with_message<N>(self, message: N) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>,
    {
        Self {
            message: Some(message.into()),
            ..self
        }
    }

    fn with_param<N, T>(mut self, name: N, val: &T) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>,
        T: Serialize,
    {
        self.add_param(name.into(), val);
        self
    }

    fn into_constraint_error(self, field: &'static str) -> InputError
    where
        Self: Sized,
    {
        let mut error = ValidationErrors::new();
        error.add(field, self);
        InputError::Constraint(error)
    }
}

#[derive(Debug, ThisError)]
pub enum InputError {
    #[error("Path could not be parsed for input")]
    PathFormat(PathRejection),
    #[error("Query could not be parsed for input")]
    QueryFormat(QueryRejection),
    #[error("Body could not be parsed for input")]
    JsonFormat(JsonRejection),
    #[error("Input constraint violated")]
    Constraint(ValidationErrors),
}

impl From<InputError> for Problem {
    fn from(value: InputError) -> Self {
        match value {
            InputError::PathFormat(err) => Problem::bad_request(problems::INPUT_PATH).with_detail(format!("{err:?}")),
            InputError::QueryFormat(err) => Problem::bad_request(problems::INPUT_QUERY).with_detail(format!("{err}")),
            InputError::JsonFormat(JsonRejection::JsonSyntaxError(err)) => {
                Problem::bad_request(problems::INPUT_BODY).with_detail(err.body_text())
            }
            InputError::JsonFormat(JsonRejection::JsonDataError(err)) => {
                //todo: convert it into validation error
                Problem::bad_request(problems::INPUT_BODY).with_detail(err.body_text())
            }
            InputError::Constraint(mut detail) => {
                strip_value_params(&mut detail);
                Problem::bad_request(problems::INPUT_VALIDATION)
                    .with_detail("Input validation failed")
                    .with_extension(detail)
            }
            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}

pub struct ValidatedPath<T>(pub T)
where
    T: DeserializeOwned + Validate + 'static;

impl<S, T> FromRequestParts<S> for ValidatedPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send + Validate,
{
    type Rejection = ErrorResponse<InputError>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");

        let Path(data) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|err| ErrorResponse::new(&problem_config, InputError::PathFormat(err)))?;
        data.validate()
            .map_err(|err| ErrorResponse::new(&problem_config, InputError::Constraint(err)))?;
        Ok(Self(data))
    }
}

pub struct ValidatedQuery<T>(pub T)
where
    T: DeserializeOwned + Validate + 'static;

impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate + 'static,
{
    type Rejection = ErrorResponse<InputError>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");

        let Query(data) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|err| ErrorResponse::new(&problem_config, InputError::QueryFormat(err)))?;
        data.validate()
            .map_err(|err| ErrorResponse::new(&problem_config, InputError::Constraint(err)))?;
        Ok(Self(data))
    }
}

pub struct ValidatedJson<J>(pub J)
where
    J: Validate + 'static;

impl<S, J> FromRequest<S> for ValidatedJson<J>
where
    S: Send + Sync,
    J: Validate + 'static,
    Json<J>: FromRequest<(), Rejection = JsonRejection>,
{
    type Rejection = ErrorResponse<InputError>;

    async fn from_request(mut req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = req
            .extract_parts::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");

        let Json(data) = req
            .extract::<Json<J>, _>()
            .await
            .map_err(|err| ErrorResponse::new(&problem_config, InputError::JsonFormat(err)))?;
        data.validate()
            .map_err(|err| ErrorResponse::new(&problem_config, InputError::Constraint(err)))?;
        Ok(Self(data))
    }
}

#[cfg(test)]
mod tests {
    use super::strip_value_params;
    use std::collections::BTreeMap;
    use validator::{ValidationError, ValidationErrors, ValidationErrorsKind};

    fn error_with_value(code: &'static str) -> ValidationError {
        let mut error = ValidationError::new(code);
        error.add_param("value".into(), &vec!["a", "b", "c"]);
        error.add_param("max".into(), &100);
        error
    }

    fn params_of<'a>(errors: &'a ValidationErrors, field: &str) -> &'a ValidationError {
        match errors.errors().get(field).unwrap() {
            ValidationErrorsKind::Field(list) => &list[0],
            _ => panic!("expected field errors"),
        }
    }

    #[test]
    fn strips_value_keeps_other_params_on_field_errors() {
        let mut errors = ValidationErrors::new();
        errors.add("user_ids", error_with_value("length"));

        strip_value_params(&mut errors);

        let error = params_of(&errors, "user_ids");
        assert!(!error.params.contains_key("value"), "value must be removed");
        assert!(error.params.contains_key("max"), "other params must be kept");
    }

    #[test]
    fn strips_value_in_nested_struct_and_list() {
        let mut nested = ValidationErrors::new();
        nested.add("inner", error_with_value("length"));

        let mut list_item = ValidationErrors::new();
        list_item.add("elem", error_with_value("range"));
        let mut list = BTreeMap::new();
        list.insert(0usize, Box::new(list_item));

        let mut errors = ValidationErrors::new();
        errors
            .errors_mut()
            .insert("child".into(), ValidationErrorsKind::Struct(Box::new(nested)));
        errors
            .errors_mut()
            .insert("items".into(), ValidationErrorsKind::List(list));

        strip_value_params(&mut errors);

        match errors.errors().get("child").unwrap() {
            ValidationErrorsKind::Struct(nested) => {
                assert!(!params_of(nested, "inner").params.contains_key("value"));
            }
            _ => panic!("expected struct"),
        }
        match errors.errors().get("items").unwrap() {
            ValidationErrorsKind::List(list) => {
                assert!(!params_of(list.get(&0).unwrap(), "elem").params.contains_key("value"));
            }
            _ => panic!("expected list"),
        }
    }
}
