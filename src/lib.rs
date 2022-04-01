#![doc = include_str!("../README.md")]

use std::collections::HashSet;

use serde_json::*;

type Document = Map<String, Value>;

const SPLIT_SYMBOL: char = '.';

/// Returns `true` if the `subset` is contained in the `main` string.
fn contained_in(subset: &str, main: &str) -> bool {
    subset.starts_with(main)
        && subset[main.len()..]
            .chars()
            .next()
            .map(|c| c == SPLIT_SYMBOL)
            .unwrap_or(true)
}

/// This function is not strictly needed but it should improve the performance
/// in the case of a user including one field twice.
fn simplify_selectors(mut selectors: Vec<String>) -> Vec<String> {
    // order the field like that; [person, person.age, person.name]
    selectors.sort();
    // remove the selectors included in the previous selector; [person]
    // be cautious, dedup_by give you the elements in the wrong order.
    selectors.dedup_by(|sub, main| contained_in(sub, main));
    selectors
}

/// Permissively selects values in a json with a list of selectors.
/// Returns a new json containing all the selected fields.
/// ```
/// use serde_json::*;
/// use permissive_json_pointer::select_values;
///
/// let value: Value = json!({
///     "name": "peanut",
///     "age": 8,
///     "race": {
///         "name": "bernese mountain",
///         "avg_age": 12,
///         "size": "80cm",
///     },
/// });
/// let value: &Map<String, Value> = value.as_object().unwrap();
///
/// let res: Value = select_values(value, vec!["name".to_string(), "race.name".to_string()]).into();
/// assert_eq!(
///     res,
///     json!({
///         "name": "peanut",
///         "race": {
///             "name": "bernese mountain",
///         },
///     })
/// );
/// ```
pub fn select_values(value: &Map<String, Value>, selectors: Vec<String>) -> Map<String, Value> {
    let selectors = simplify_selectors(selectors);
    let selectors = selectors.iter().map(|s| s.as_ref()).collect();
    create_value(value, selectors)
}

fn create_value(value: &Document, mut selectors: HashSet<&str>) -> Document {
    let mut new_value: Document = Map::new();

    for (key, value) in value.iter() {
        // first we insert all the key at the root level
        if selectors.contains(key as &str) {
            new_value.insert(key.to_string(), value.clone());
            // if the key was simple we can delete it and move to
            // the next key
            if is_simple(key) {
                selectors.remove(key as &str);
                continue;
            }
        }

        // we extract all the sub selectors matching the current field
        // if there was [person.name, person.age] and if we are on the field
        // `person`. Then we generate the following sub selectors: [name, age].
        let sub_selectors: HashSet<&str> = selectors
            .iter()
            .filter(|s| contained_in(s, key))
            .filter_map(|s| s.trim_start_matches(key).get(SPLIT_SYMBOL.len_utf8()..))
            .collect();

        if !sub_selectors.is_empty() {
            match value {
                Value::Array(array) => {
                    let array = create_array(array, &sub_selectors);
                    if !array.is_empty() {
                        new_value.insert(key.to_string(), array.into());
                    }
                }
                Value::Object(object) => {
                    let object = create_value(object, sub_selectors);
                    if !object.is_empty() {
                        new_value.insert(key.to_string(), object.into());
                    }
                }
                _ => (),
            }
        }
    }

    new_value
}

fn create_array(array: &Vec<Value>, selectors: &HashSet<&str>) -> Vec<Value> {
    let mut res = Vec::new();

    for value in array {
        match value {
            Value::Array(array) => {
                let array = create_array(array, selectors);
                if !array.is_empty() {
                    res.push(array.into());
                }
            }
            Value::Object(object) => {
                let object = create_value(object, selectors.clone());
                if !object.is_empty() {
                    res.push(object.into());
                }
            }
            _ => (),
        }
    }

    res
}

fn is_simple(key: impl AsRef<str>) -> bool {
    !key.as_ref().contains(SPLIT_SYMBOL)
}

#[cfg(test)]
mod tests {
    use big_s::S;

    use super::*;

    #[test]
    fn test_contained_in() {
        assert!(contained_in("animaux", "animaux"));
        assert!(contained_in("animaux.chien", "animaux"));
        assert!(contained_in(
            "animaux.chien.race.bouvier bernois.fourrure.couleur",
            "animaux"
        ));
        assert!(contained_in(
            "animaux.chien.race.bouvier bernois.fourrure.couleur",
            "animaux.chien"
        ));
        assert!(contained_in(
            "animaux.chien.race.bouvier bernois.fourrure.couleur",
            "animaux.chien.race.bouvier bernois"
        ));
        assert!(contained_in(
            "animaux.chien.race.bouvier bernois.fourrure.couleur",
            "animaux.chien.race.bouvier bernois.fourrure"
        ));
        assert!(contained_in(
            "animaux.chien.race.bouvier bernois.fourrure.couleur",
            "animaux.chien.race.bouvier bernois.fourrure.couleur"
        ));

        // -- the wrongs
        assert!(!contained_in("chien", "chat"));
        assert!(!contained_in("animaux", "animaux.chien"));
        assert!(!contained_in("animaux.chien", "animaux.chat"));

        // -- the strange edge cases
        assert!(!contained_in("animaux.chien", "anima"));
        assert!(!contained_in("animaux.chien", "animau"));
        assert!(!contained_in("animaux.chien", "animaux."));
        assert!(!contained_in("animaux.chien", "animaux.c"));
        assert!(!contained_in("animaux.chien", "animaux.ch"));
        assert!(!contained_in("animaux.chien", "animaux.chi"));
        assert!(!contained_in("animaux.chien", "animaux.chie"));
    }

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

        println!("RIGHT BEFORE");

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

        let res: Value = select_values(
            value,
            vec![
                S("doggos.marc.race.name"),
                S("doggos.marc.age"),
                S("doggos.jean.race.name"),
                S("other.field"),
            ],
        )
        .into();

        assert_eq!(
            res,
            json!({
                "doggos": [
                    {
                        "jean": {
                            "race": {
                                "name": "bernese mountain",
                            }
                        }
                    },
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

    #[test]
    fn all_conflict_variation() {
        let value: Value = json!({
           "pet.dog.name": "jean",
           "pet.dog": {
             "name": "bob"
           },
           "pet": {
             "dog.name": "michel"
           },
           "pet": {
             "dog": {
               "name": "milan"
             }
           }
        });
        let value: &Document = value.as_object().unwrap();

        let res: Value = select_values(value, vec![S("pet.dog.name")]).into();
        assert_eq!(
            res,
            json!({
               "pet.dog.name": "jean",
               "pet.dog": {
                 "name": "bob"
               },
               "pet": {
                 "dog.name": "michel"
               },
               "pet": {
                 "dog": {
                   "name": "milan"
                 }
               }
            })
        );
    }
}
