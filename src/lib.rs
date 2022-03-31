use serde_json::*;

type Document = Map<String, Value>;

const SPLIT_SYMBOL: char = '.';

fn contained_in(main: &str, sub: &str) -> bool {
    main.starts_with(sub)
        && main[sub.len()..]
            .chars()
            .next()
            .map(|c| c == '.')
            .unwrap_or(true)
}

/// This function is not strictly needed but it should improve the performance
/// in the case of a user including one field twice.
fn simplify_selectors(mut selectors: Vec<String>) -> Vec<String> {
    // order the field like that; [person, person.age, person.name]
    selectors.sort();
    // remove the selectors included in the previous selector; [person]
    selectors.dedup_by(|sub, main| contained_in(sub, main));
    selectors
}

pub fn select_values(value: &Document, selectors: Vec<String>) -> Document {
    let mut new_value: Document = Map::new();

    let selectors = simplify_selectors(selectors);

    let (complex, simple) = selectors
        .into_iter()
        .partition::<Vec<String>, _>(|s| is_complex(s));

    for selector in simple {
        if let Some(value) = value.get(&selector) {
            new_value.insert(selector, value.clone());
        }
    }

    for selector in complex {
        let complex = select_complex_value(value, &selector);

        for (key, value) in complex {
            if let Some(new_value) = new_value.get_mut(&key) {
                merge_value(new_value, &value);
            } else {
                new_value.insert(key.to_string(), value.into());
            }
        }
    }

    new_value
}

fn select_complex_value(value: &Document, selector: &str) -> Document {
    let mut new_value: Document = Map::new();

    // this happens if there was field containing a `.`
    // not in an else branch because we can have both
    if let Some(value) = value.get(selector) {
        new_value.insert(selector.to_string(), value.clone());
    }

    for (idx, _) in selector.match_indices(SPLIT_SYMBOL) {
        let outer_key = &selector[..idx];
        let inner_key = &selector[idx + SPLIT_SYMBOL.len_utf8()..];

        if let Some(value) = value.get(outer_key) {
            match value {
                Value::Array(arr) => {
                    let array = select_in_array(arr, inner_key);
                    new_value.insert(outer_key.to_string(), array.into());
                }
                Value::Object(obj) => {
                    let value = select_complex_value(obj, inner_key);
                    if let Some(_value) = new_value.get_mut(outer_key) {
                        todo!();
                    } else {
                        new_value.insert(outer_key.to_string(), value.into());
                    }
                }
                _ => (),
            }
        }
    }

    new_value
}

fn select_in_array(array: &[Value], selector: &str) -> Vec<Value> {
    let mut new_values = Vec::new();

    for value in array {
        match value {
            Value::Array(arr) => {
                let mut array = select_in_array(arr, selector);
                new_values.append(&mut array);
            }
            Value::Object(obj) => {
                let value = select_complex_value(obj, selector);
                if !value.is_empty() {
                    new_values.push(value.into());
                }
            }
            _ => (),
        }
    }

    new_values
}

fn merge_value(base: &mut Value, other: &Value) {
    match (base, other) {
        (Value::Array(base), Value::Array(other)) => base.append(&mut other.clone()),
        (Value::Object(base), Value::Object(other)) => {
            for (key, value) in other {
                base.insert(key.to_string(), value.clone());
            }
        }
        _ => panic!("unexpected"),
    }
}

fn is_complex(key: impl AsRef<str>) -> bool {
    key.as_ref().contains(SPLIT_SYMBOL)
}

#[cfg(test)]
mod tests {
    use big_s::S;

    use super::*;

    #[test]
    fn test_simplify_selectors() {
        assert_eq!(
            simplify_selectors(vec![S("person.name"), S("person.dog")]),
            vec![S("person.dog"), S("person.name"),],
        );
        assert_eq!(
            simplify_selectors(vec![S("person"), S("person.name"), S("person.dog")]),
            vec![S("person")],
        );
        assert_eq!(
            simplify_selectors(vec![S("person.name"), S("person"), S("person.dog")]),
            vec![S("person")],
        );
        assert_eq!(
            simplify_selectors(vec![S("person.name"), S("person.dog"), S("person")]),
            vec![S("person")],
        );
        assert_eq!(
            simplify_selectors(vec![S("family.brother.dog"), S("family.brother")]),
            vec![S("family.brother")],
        );
        assert_eq!(
            simplify_selectors(vec![
                S("family.brother.dog"),
                S("family.brother"),
                S("family.brother.cat")
            ]),
            vec![S("family.brother")],
        );
    }

    #[test]
    fn simple_key() {
        let value: Value = json!({
            "name": "peanut",
            "age": 8,
            "race": {
                "name": "bernese mountain",
                "avg_age": 12,
                "size": "80cm",
            }
        });
        let value: &Document = value.as_object().unwrap();

        let res: Value = select_values(value, vec![S("name")]).into();
        assert_eq!(
            res,
            json!({
                "name": "peanut",
            })
        );

        let res: Value = select_values(value, vec![S("age")]).into();
        assert_eq!(
            res,
            json!({
                "age": 8,
            })
        );

        let res: Value = select_values(value, vec![S("name"), S("age")]).into();
        assert_eq!(
            res,
            json!({
                "name": "peanut",
                "age": 8,
            })
        );

        let res: Value = select_values(value, vec![S("race")]).into();
        assert_eq!(
            res,
            json!({
                "race": {
                    "name": "bernese mountain",
                    "avg_age": 12,
                    "size": "80cm",
                }
            })
        );

        let res: Value = select_values(value, vec![S("name"), S("age"), S("race")]).into();
        assert_eq!(
            res,
            json!({
                "name": "peanut",
                "age": 8,
                "race": {
                    "name": "bernese mountain",
                    "avg_age": 12,
                    "size": "80cm",
                }
            })
        );
    }

    #[test]
    fn complex_key() {
        let value: Value = json!({
            "name": "peanut",
            "age": 8,
            "race": {
                "name": "bernese mountain",
                "avg_age": 12,
                "size": "80cm",
            }
        });
        let value: &Document = value.as_object().unwrap();

        let res: Value = select_values(value, vec![S("race")]).into();
        assert_eq!(
            res,
            json!({
                "race": {
                    "name": "bernese mountain",
                    "avg_age": 12,
                    "size": "80cm",
                }
            })
        );

        let res: Value = select_values(value, vec![S("race.name")]).into();
        assert_eq!(
            res,
            json!({
                "race": {
                    "name": "bernese mountain",
                }
            })
        );

        let res: Value = select_values(value, vec![S("race.name"), S("race.size")]).into();
        assert_eq!(
            res,
            json!({
                "race": {
                    "name": "bernese mountain",
                    "size": "80cm",
                }
            })
        );

        let res: Value = select_values(
            value,
            vec![
                S("race.name"),
                S("race.size"),
                S("race.avg_age"),
                S("race.size"),
                S("age"),
            ],
        )
        .into();
        assert_eq!(
            res,
            json!({
                "age": 8,
                "race": {
                    "name": "bernese mountain",
                    "avg_age": 12,
                    "size": "80cm",
                }
            })
        );

        let res: Value = select_values(value, vec![S("race.name"), S("race")]).into();
        assert_eq!(
            res,
            json!({
                "race": {
                    "name": "bernese mountain",
                    "avg_age": 12,
                    "size": "80cm",
                }
            })
        );

        let res: Value = select_values(value, vec![S("race"), S("race.name")]).into();
        assert_eq!(
            res,
            json!({
                "race": {
                    "name": "bernese mountain",
                    "avg_age": 12,
                    "size": "80cm",
                }
            })
        );
    }

    #[test]
    fn multi_level_nested() {
        let value: Value = json!({
            "jean": {
                "age": 8,
                "race": {
                    "name": "bernese mountain",
                    "size": "80cm",
                }
            }
        });
        let value: &Document = value.as_object().unwrap();

        let res: Value = select_values(value, vec![S("jean")]).into();
        assert_eq!(
            res,
            json!({
                "jean": {
                    "age": 8,
                    "race": {
                        "name": "bernese mountain",
                        "size": "80cm",
                    }
                }
            })
        );

        let res: Value = select_values(value, vec![S("jean.age")]).into();
        assert_eq!(
            res,
            json!({
                "jean": {
                    "age": 8,
                }
            })
        );

        let res: Value = select_values(value, vec![S("jean.race.size")]).into();
        assert_eq!(
            res,
            json!({
                "jean": {
                    "race": {
                        "size": "80cm",
                    }
                }
            })
        );

        let res: Value = select_values(value, vec![S("jean.race.name"), S("jean.age")]).into();
        assert_eq!(
            res,
            json!({
                "jean": {
                    "age": 8,
                    "race": {
                        "name": "bernese mountain",
                    }
                }
            })
        );

        let res: Value = select_values(value, vec![S("jean.race")]).into();
        assert_eq!(
            res,
            json!({
                "jean": {
                    "race": {
                        "name": "bernese mountain",
                        "size": "80cm",
                    }
                }
            })
        );
    }

    #[test]
    fn array_and_deep_nested() {
        let value: Value = json!({
            "doggos": [
                {
                    "jean": {
                        "age": 8,
                        "race": {
                            "name": "bernese mountain",
                            "size": "80cm",
                        }
                    }
                },
                {
                    "marc": {
                        "age": 4,
                        "race": {
                            "name": "golden retriever",
                            "size": "60cm",
                        }
                    }
                },
            ]
        });
        let value: &Document = value.as_object().unwrap();

        let res: Value = select_values(value, vec![S("doggos.jean")]).into();
        assert_eq!(
            res,
            json!({
                "doggos": [
                    {
                        "jean": {
                            "age": 8,
                            "race": {
                                "name": "bernese mountain",
                                "size": "80cm",
                            }
                        }
                    }
                ]
            })
        );

        let res: Value = select_values(value, vec![S("doggos.marc")]).into();
        assert_eq!(
            res,
            json!({
                "doggos": [
                    {
                        "marc": {
                            "age": 4,
                            "race": {
                                "name": "golden retriever",
                                "size": "60cm",
                            }
                        }
                    }
                ]
            })
        );

        let res: Value = select_values(value, vec![S("doggos.marc.race")]).into();
        assert_eq!(
            res,
            json!({
                "doggos": [
                    {
                        "marc": {
                            "race": {
                                "name": "golden retriever",
                                "size": "60cm",
                            }
                        }
                    }
                ]
            })
        );

        let res: Value = select_values(
            value,
            vec![S("doggos.marc.race.name"), S("doggos.marc.age")],
        )
        .into();

        println!("{}", serde_json::to_string_pretty(&res).unwrap());

        assert_eq!(
            res,
            json!({
                "doggos": [
                    {
                        "marc": {
                            "age": 4,
                            "race": {
                                "name": "golden retriever",
                            }
                        }
                    }
                ]
            })
        );
    }
}
