#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use serde_json::{Map, Value};
use std::error::Error;
use std::fmt;
use winnow::combinator::{alt, repeat};
use winnow::prelude::*;
use winnow::token::take_while;

/// A parsed HTTP header assignment like `Accept:application/json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHeader {
    /// The header name.
    pub name: String,
    /// The header value.
    pub value: String,
}

/// A parsed query parameter assignment like `page==2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQueryParam {
    /// The query parameter name.
    pub name: String,
    /// The query parameter value.
    pub value: String,
}

/// The parsed representation of a list of HTTP CLI input parts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedInput {
    /// Parsed request headers, in input order.
    pub headers: Vec<ParsedHeader>,
    /// Parsed query parameters, in input order.
    pub query_params: Vec<ParsedQueryParam>,
    /// Parsed JSON body, if any body assignments were present.
    pub body: Option<Value>,
}

/// An error returned when one of the input parts cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseInputError {
    message: String,
}

impl ParseInputError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn unexpected(input: &str) -> Self {
        Self::new(format!("unexpected input: {input:?}"))
    }

    fn invalid_json(input: &str, error: serde_json::Error) -> Self {
        Self::new(format!("invalid JSON value in {input:?}: {error}"))
    }

    fn type_mismatch(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    /// Returns the error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ParseInputError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathSegment {
    ObjectKey(String),
    ArrayIndex(usize),
    ArrayEnd,
}

#[derive(Debug, Clone, PartialEq)]
struct BodyComponent {
    path: Vec<PathSegment>,
    value: Value,
}

#[derive(Debug, Clone, PartialEq)]
enum ParsedPart {
    Header(ParsedHeader),
    Query(ParsedQueryParam),
    Body(BodyComponent),
}

/// Parses HTTP CLI input parts into headers, query params, and a JSON body.
pub fn parse_input<I, S>(parts: I) -> Result<ParsedInput, ParseInputError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut parsed = ParsedInput::default();

    for part in parts {
        match parse_part(part.as_ref())? {
            ParsedPart::Header(header) => parsed.headers.push(header),
            ParsedPart::Query(query) => parsed.query_params.push(query),
            ParsedPart::Body(component) => apply_body_component(&mut parsed.body, component)?,
        }
    }

    Ok(parsed)
}

fn parse_part(part: &str) -> Result<ParsedPart, ParseInputError> {
    if let Some(component) = parse_json_assignment(part)? {
        return Ok(ParsedPart::Body(component));
    }

    if let Some(query) = parse_query_param(part)? {
        return Ok(ParsedPart::Query(query));
    }

    if let Some(header) = parse_header(part)? {
        return Ok(ParsedPart::Header(header));
    }

    if let Some(component) = parse_string_assignment(part)? {
        return Ok(ParsedPart::Body(component));
    }

    Err(ParseInputError::unexpected(part))
}

fn parse_json_assignment(part: &str) -> Result<Option<BodyComponent>, ParseInputError> {
    let Some((path_raw, value_raw)) = part.split_once(":=") else {
        return Ok(None);
    };

    let path = parse_access_path(path_raw).map_err(|_| ParseInputError::unexpected(part))?;
    let value = serde_json::from_str(value_raw)
        .map_err(|error| ParseInputError::invalid_json(part, error))?;

    Ok(Some(BodyComponent { path, value }))
}

fn parse_query_param(part: &str) -> Result<Option<ParsedQueryParam>, ParseInputError> {
    let Some((name, value)) = part.split_once("==") else {
        return Ok(None);
    };

    if !is_valid_query_name(name) {
        return Err(ParseInputError::unexpected(part));
    }

    Ok(Some(ParsedQueryParam {
        name: name.to_string(),
        value: value.to_string(),
    }))
}

fn parse_header(part: &str) -> Result<Option<ParsedHeader>, ParseInputError> {
    let Some((name, value)) = part.split_once(':') else {
        return Ok(None);
    };

    if !is_valid_header_name(name) {
        return Err(ParseInputError::unexpected(part));
    }

    Ok(Some(ParsedHeader {
        name: name.to_string(),
        value: value.to_string(),
    }))
}

fn parse_string_assignment(part: &str) -> Result<Option<BodyComponent>, ParseInputError> {
    let Some((path_raw, value_raw)) = part.split_once('=') else {
        return Ok(None);
    };

    let path = parse_access_path(path_raw).map_err(|_| ParseInputError::unexpected(part))?;
    let value = Value::String(value_raw.to_string());

    Ok(Some(BodyComponent { path, value }))
}

fn is_valid_header_name(name: &str) -> bool {
    let mut input = name;
    take_while::<_, _, ()>(1.., is_header_name_char)
        .parse_next(&mut input)
        .is_ok()
        && input.is_empty()
}

fn is_header_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn is_valid_query_name(name: &str) -> bool {
    let mut input = name;
    take_while::<_, _, ()>(1.., is_query_name_char)
        .parse_next(&mut input)
        .is_ok()
        && input.is_empty()
}

fn is_query_name_char(c: char) -> bool {
    c != '='
}

fn parse_access_path(path_raw: &str) -> Result<Vec<PathSegment>, ParseInputError> {
    let mut input = path_raw;
    let path = repeat(1.., access_path_segment)
        .parse_next(&mut input)
        .map_err(|_| ParseInputError::unexpected(path_raw))?;

    if !input.is_empty() {
        return Err(ParseInputError::unexpected(path_raw));
    }

    Ok(path)
}

fn access_path_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    alt((array_index_segment, object_key_segment, array_end_segment)).parse_next(input)
}

fn array_end_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    "[]".parse_next(input)?;
    Ok(PathSegment::ArrayEnd)
}

fn array_index_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    alt((
        bracket_array_index_segment,
        dotted_array_index_segment,
        bare_array_index_segment,
    ))
    .parse_next(input)
}

fn bracket_array_index_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    "[".parse_next(input)?;
    let index = parse_index_digits(input)?;
    "]".parse_next(input)?;
    Ok(PathSegment::ArrayIndex(index))
}

fn dotted_array_index_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    ".".parse_next(input)?;
    let index = parse_index_digits(input)?;
    Ok(PathSegment::ArrayIndex(index))
}

fn bare_array_index_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    let index = parse_index_digits(input)?;
    Ok(PathSegment::ArrayIndex(index))
}

fn parse_index_digits(input: &mut &str) -> winnow::Result<usize> {
    let digits: &str = take_while(1.., |c: char| c.is_ascii_digit()).parse_next(input)?;
    Ok(digits.parse().unwrap_or(usize::MAX))
}

fn object_key_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    alt((
        bracket_object_key_segment,
        dotted_object_key_segment,
        bare_object_key_segment,
    ))
    .parse_next(input)
}

fn bracket_object_key_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    "[".parse_next(input)?;
    let key: &str = take_while(1.., |c: char| c != ']').parse_next(input)?;
    "]".parse_next(input)?;
    Ok(PathSegment::ObjectKey(key.to_string()))
}

fn dotted_object_key_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    ".".parse_next(input)?;
    let key: &str = take_while(1.., is_plain_object_key_char).parse_next(input)?;
    Ok(PathSegment::ObjectKey(key.to_string()))
}

fn bare_object_key_segment(input: &mut &str) -> winnow::Result<PathSegment> {
    let key: &str = take_while(1.., is_plain_object_key_char).parse_next(input)?;
    Ok(PathSegment::ObjectKey(key.to_string()))
}

fn is_plain_object_key_char(c: char) -> bool {
    c != '.' && c != '[' && c != ':' && c != '='
}

fn apply_body_component(
    body: &mut Option<Value>,
    component: BodyComponent,
) -> Result<(), ParseInputError> {
    if body.is_none() {
        *body = Some(Value::Null);
    }

    let Some(target) = body.as_mut() else {
        return Err(ParseInputError::type_mismatch(
            "body was unexpectedly absent",
        ));
    };

    set_path_value(target, &component.path, component.value)
}

fn set_path_value(
    target: &mut Value,
    path: &[PathSegment],
    value: Value,
) -> Result<(), ParseInputError> {
    if path.is_empty() {
        *target = value;
        return Ok(());
    }

    match &path[0] {
        PathSegment::ObjectKey(key) => {
            if target.is_null() {
                *target = Value::Object(Map::new());
            }

            let Some(map) = target.as_object_mut() else {
                return Err(ParseInputError::type_mismatch(format!(
                    "attempted to access key of non-object ({}): {}",
                    value_type_name(target),
                    target
                )));
            };

            let entry = map.entry(key.clone()).or_insert(Value::Null);
            set_path_value(entry, &path[1..], value)
        }
        PathSegment::ArrayIndex(index) => {
            if target.is_null() {
                *target = Value::Array(Vec::new());
            }

            let Some(array) = target.as_array_mut() else {
                return Err(ParseInputError::type_mismatch(format!(
                    "attempted to access index of non-array ({}): {}",
                    value_type_name(target),
                    target
                )));
            };

            if *index >= array.len() {
                array.resize(*index + 1, Value::Null);
            }

            set_path_value(&mut array[*index], &path[1..], value)
        }
        PathSegment::ArrayEnd => {
            if target.is_null() {
                *target = Value::Array(Vec::new());
            }

            let Some(array) = target.as_array_mut() else {
                return Err(ParseInputError::type_mismatch(format!(
                    "attempted to access end of non-array ({}): {}",
                    value_type_name(target),
                    target
                )));
            };

            array.push(Value::Null);
            let index = array.len() - 1;
            set_path_value(&mut array[index], &path[1..], value)
        }
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
