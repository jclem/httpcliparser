use httpcliparser::{parse_input, ParseInputError, ParsedHeader, ParsedInput, ParsedQueryParam};
use serde_json::json;

fn parse(parts: &[&str]) -> Result<ParsedInput, ParseInputError> {
    parse_input(parts.iter().copied())
}

#[test]
fn parses_headers_query_and_body_components() {
    let parsed = parse(&[
        "Authorization:Bearer token",
        "q==hello world",
        "foo[bar]=baz",
        "is_draft:=true",
    ])
    .expect("parse input");

    assert_eq!(
        parsed,
        ParsedInput {
            headers: vec![ParsedHeader {
                name: "Authorization".to_string(),
                value: "Bearer token".to_string(),
            }],
            query_params: vec![ParsedQueryParam {
                name: "q".to_string(),
                value: "hello world".to_string(),
            }],
            body: Some(json!({
                "foo": {"bar": "baz"},
                "is_draft": true,
            })),
        }
    );
}

#[test]
fn parses_simple_header() {
    let parsed = parse(&["foo:bar"]).expect("parse input");
    assert_eq!(
        parsed,
        ParsedInput {
            headers: vec![ParsedHeader {
                name: "foo".to_string(),
                value: "bar".to_string(),
            }],
            query_params: vec![],
            body: None,
        }
    );
}

#[test]
fn parses_quoted_header() {
    let parsed = parse(&["foo:bar baz"]).expect("parse input");
    assert_eq!(
        parsed.headers,
        vec![ParsedHeader {
            name: "foo".to_string(),
            value: "bar baz".to_string(),
        }]
    );
}

#[test]
fn errors_on_disallowed_header_char() {
    let err = parse(&["foo bar:baz"]).expect_err("expected error");
    assert_eq!(err.to_string(), "unexpected input: \"foo bar:baz\"");
}

#[test]
fn parses_simple_query_param() {
    let parsed = parse(&["foo==bar"]).expect("parse input");
    assert_eq!(
        parsed.query_params,
        vec![ParsedQueryParam {
            name: "foo".to_string(),
            value: "bar".to_string(),
        }]
    );
}

#[test]
fn parses_quoted_query_param() {
    let parsed = parse(&["foo bar==bar baz"]).expect("parse input");
    assert_eq!(
        parsed.query_params,
        vec![ParsedQueryParam {
            name: "foo bar".to_string(),
            value: "bar baz".to_string(),
        }]
    );
}

#[test]
fn parses_simple_kv_body_param() {
    let parsed = parse(&["foo=bar"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": "bar"})));
}

#[test]
fn parses_nested_kv_body_param() {
    let parsed = parse(&["foo[bar]=baz"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": {"bar": "baz"}})));
}

#[test]
fn parses_multi_nested_kv_body_param() {
    let parsed = parse(&["foo[bar][baz][qux]=quux"]).expect("parse input");
    assert_eq!(
        parsed.body,
        Some(json!({
            "foo": {
                "bar": {
                    "baz": {
                        "qux": "quux"
                    }
                }
            }
        }))
    );
}

#[test]
fn parses_array_end_param() {
    let parsed = parse(&["[]=foo"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!(["foo"])));
}

#[test]
fn parses_nested_array_end_param() {
    let parsed = parse(&["foo[][]=bar"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": [["bar"]]})));
}

#[test]
fn parses_array_index_param() {
    let parsed = parse(&["[1]=foo"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!([null, "foo"])));
}

#[test]
fn parses_array_index_param_overwrite() {
    let parsed = parse(&["[1]=foo", "[1]=bar"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!([null, "bar"])));
}

#[test]
fn parses_nested_array_index_param() {
    let parsed = parse(&["foo[0][0]=bar"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": [["bar"]]})));
}

#[test]
fn parses_complex_param() {
    let parsed = parse(&["foo[][bar]=baz"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": [{"bar": "baz"}]})));
}

#[test]
fn parses_multiple_complex_params() {
    let parsed = parse(&[
        "foo[][bar]=baz",
        "foo[][qux]=quux",
        "foo[3][][][a][][4][][b][c][][d]=x",
    ])
    .expect("parse input");

    assert_eq!(
        parsed.body,
        Some(json!({
            "foo": [
                {"bar": "baz"},
                {"qux": "quux"},
                null,
                [
                    [
                        {
                            "a": [
                                [null, null, null, null, [
                                    {
                                        "b": {
                                            "c": [
                                                {"d": "x"}
                                            ]
                                        }
                                    }
                                ]]
                            ]
                        }
                    ]
                ]
            ]
        }))
    );
}

#[test]
fn parses_multiple_complex_params_flexible() {
    let parsed = parse(&[
        "foo[].bar=baz",
        "foo[]qux=quux",
        "foo.3[][]a[]4[].b[c][][d]=x",
    ])
    .expect("parse input");

    assert_eq!(
        parsed.body,
        Some(json!({
            "foo": [
                {"bar": "baz"},
                {"qux": "quux"},
                null,
                [
                    [
                        {
                            "a": [
                                [null, null, null, null, [
                                    {
                                        "b": {
                                            "c": [
                                                {"d": "x"}
                                            ]
                                        }
                                    }
                                ]]
                            ]
                        }
                    ]
                ]
            ]
        }))
    );
}

#[test]
fn parses_raw_json_maps() {
    let parsed = parse(&["foo:={\"bar\":\"baz\"}"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": {"bar": "baz"}})));
}

#[test]
fn parses_raw_json_strings() {
    let parsed = parse(&["foo:=\"bar\""]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": "bar"})));
}

#[test]
fn parses_raw_json_ints() {
    let parsed = parse(&["foo:=1"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": 1})));
}

#[test]
fn parses_raw_json_nulls() {
    let parsed = parse(&["foo:=null"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": null})));
}

#[test]
fn sets_multiple_array_end() {
    let parsed = parse(&["foo[]=bar", "foo[]=baz"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": ["bar", "baz"]})));
}

#[test]
fn sets_multiple_array_index() {
    let parsed = parse(&["foo[]=bar", "foo[]=baz", "foo[2]=qux"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": ["bar", "baz", "qux"]})));
}

#[test]
fn gives_priority_to_json_then_query_then_header_then_kv() {
    let parsed = parse(&[
        "foo:=true",
        "bar==baz",
        "Authorization:Bearer token",
        "qux=value",
    ])
    .expect("parse input");

    assert_eq!(
        parsed.query_params,
        vec![ParsedQueryParam {
            name: "bar".to_string(),
            value: "baz".to_string(),
        }]
    );
    assert_eq!(
        parsed.headers,
        vec![ParsedHeader {
            name: "Authorization".to_string(),
            value: "Bearer token".to_string(),
        }]
    );
    assert_eq!(
        parsed.body,
        Some(json!({
            "foo": true,
            "qux": "value",
        }))
    );
}

#[test]
fn errors_on_invalid_json_value() {
    let err = parse(&["foo:={bar"]).expect_err("expected error");
    assert!(
        err.to_string()
            .starts_with("invalid JSON value in \"foo:={bar\""),
        "unexpected error: {err}"
    );
}

#[test]
fn errors_on_unexpected_remainder() {
    let err = parse(&["foo[bar=baz"]).expect_err("expected error");
    assert_eq!(err.to_string(), "unexpected input: \"foo[bar=baz\"");
}

#[test]
fn preserves_sparse_array_slots_as_null() {
    let parsed = parse(&["foo[3]=bar"]).expect("parse input");
    let body = parsed.body.expect("body");
    assert_eq!(body, json!({"foo": [null, null, null, "bar"]}));
}

#[test]
fn allows_repeated_query_params() {
    let parsed = parse(&["q==first", "q==second"]).expect("parse input");
    assert_eq!(
        parsed.query_params,
        vec![
            ParsedQueryParam {
                name: "q".to_string(),
                value: "first".to_string(),
            },
            ParsedQueryParam {
                name: "q".to_string(),
                value: "second".to_string(),
            }
        ]
    );
}

#[test]
fn overwrites_same_path_with_last_value() {
    let parsed = parse(&["foo=bar", "foo=baz"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": "baz"})));
}

#[test]
fn reports_type_mismatch_for_invalid_traversal() {
    let err = parse(&["foo=bar", "foo[0]=baz"]).expect_err("expected error");
    assert!(
        err.to_string()
            .starts_with("attempted to access index of non-array"),
        "unexpected error: {err}"
    );
}

#[test]
fn empty_body_when_only_headers_and_query() {
    let parsed = parse(&["Accept:application/json", "page==1"]).expect("parse input");
    assert_eq!(
        parsed,
        ParsedInput {
            headers: vec![ParsedHeader {
                name: "Accept".to_string(),
                value: "application/json".to_string(),
            }],
            query_params: vec![ParsedQueryParam {
                name: "page".to_string(),
                value: "1".to_string(),
            }],
            body: None,
        }
    );
}

#[test]
fn parses_json_arrays_and_numbers_without_quotes() {
    let parsed = parse(&["items:=[1,2,3]", "rating:=4.2"]).expect("parse input");
    assert_eq!(
        parsed.body,
        Some(json!({
            "items": [1, 2, 3],
            "rating": 4.2
        }))
    );
}

#[test]
fn keeps_string_assignment_values_as_raw_strings() {
    let parsed = parse(&["foo=true", "bar=123"]).expect("parse input");
    assert_eq!(
        parsed.body,
        Some(json!({
            "foo": "true",
            "bar": "123"
        }))
    );
}

#[test]
fn supports_root_value_overwrite() {
    let parsed = parse(&["[]=foo", "[]=bar"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!(["foo", "bar"])));
}

#[test]
fn parses_nested_object_and_array_mix() {
    let parsed = parse(&["root[0].user[name]=alex"]).expect("parse input");
    assert_eq!(
        parsed.body,
        Some(json!({
            "root": [
                {
                    "user": {
                        "name": "alex"
                    }
                }
            ]
        }))
    );
}

#[test]
fn query_name_must_not_be_empty() {
    let err = parse(&["==value"]).expect_err("expected error");
    assert_eq!(err.to_string(), "unexpected input: \"==value\"");
}

#[test]
fn header_name_must_match_allowed_characters() {
    let err = parse(&["hello/world:ok"]).expect_err("expected error");
    assert_eq!(err.to_string(), "unexpected input: \"hello/world:ok\"");
}

#[test]
fn json_assignment_rejects_invalid_path() {
    let err = parse(&["foo[:=1"]).expect_err("expected error");
    assert_eq!(err.to_string(), "unexpected input: \"foo[:=1\"");
}

#[test]
fn parse_returns_none_body_when_no_parts() {
    let parsed = parse(&[]).expect("parse input");
    assert_eq!(parsed.body, None);
    assert_eq!(parsed.headers, Vec::<ParsedHeader>::new());
    assert_eq!(parsed.query_params, Vec::<ParsedQueryParam>::new());
}

#[test]
fn array_indices_use_numeric_segments() {
    let parsed = parse(&["foo.0=bar", "foo.1=baz"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": ["bar", "baz"]})));
}

#[test]
fn bracket_keys_allow_periods() {
    let parsed = parse(&["foo[bar.baz]=qux"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": {"bar.baz": "qux"}})));
}

#[test]
fn supports_mixed_assignments_in_single_payload() {
    let parsed =
        parse(&["title=hello", "meta[count]:=2", "meta[tags][]=rust"]).expect("parse input");
    assert_eq!(
        parsed.body,
        Some(json!({
            "title": "hello",
            "meta": {
                "count": 2,
                "tags": ["rust"]
            }
        }))
    );
}

#[test]
fn json_null_can_be_replaced_by_nested_object_assignment() {
    let parsed = parse(&["foo:=null", "foo[bar]=baz"]).expect("parse input");
    assert_eq!(parsed.body, Some(json!({"foo": {"bar": "baz"}})));
}
