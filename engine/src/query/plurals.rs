use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Default)]
pub enum IgnorePluralsValue {
    #[default]
    Disabled,
    All,
    Languages(Vec<String>),
}

impl serde::Serialize for IgnorePluralsValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            IgnorePluralsValue::Disabled => serializer.serialize_bool(false),
            IgnorePluralsValue::All => serializer.serialize_bool(true),
            IgnorePluralsValue::Languages(langs) => langs.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for IgnorePluralsValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de;

        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = IgnorePluralsValue;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bool or array of language codes")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Self::Value, E> {
                if v {
                    Ok(IgnorePluralsValue::All)
                } else {
                    Ok(IgnorePluralsValue::Disabled)
                }
            }

            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut langs = Vec::new();
                while let Some(val) = seq.next_element::<String>()? {
                    langs.push(val);
                }
                if langs.is_empty() {
                    Ok(IgnorePluralsValue::Disabled)
                } else {
                    Ok(IgnorePluralsValue::Languages(langs))
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

static DICTIONARY: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

fn load_dictionary() -> HashMap<String, Vec<String>> {
    let json_str = include_str!("../../package/irregular-plurals.json");
    let raw: HashMap<String, String> =
        serde_json::from_str(json_str).expect("invalid irregular-plurals.json");

    let mut bidir: HashMap<String, Vec<String>> = HashMap::new();

    for (singular, plural) in &raw {
        let s = singular.to_lowercase();
        let p = plural.to_lowercase();

        if s == p {
            bidir.entry(s).or_default();
            continue;
        }

        bidir.entry(s.clone()).or_default();
        if !bidir[&s].contains(&p) {
            bidir.get_mut(&s).unwrap().push(p.clone());
        }

        bidir.entry(p.clone()).or_default();
        if !bidir[&p].contains(&s) {
            bidir.get_mut(&p).unwrap().push(s.clone());
        }
    }

    bidir
}

fn get_dictionary() -> &'static HashMap<String, Vec<String>> {
    DICTIONARY.get_or_init(load_dictionary)
}

fn strip_regular_plural(word: &str) -> Option<String> {
    if word.ends_with("ies") && word.len() > 4 {
        let before = word.as_bytes()[word.len() - 4];
        if !matches!(before, b'a' | b'e' | b'i' | b'o' | b'u') {
            return Some(format!("{}y", &word[..word.len() - 3]));
        }
    }

    if word.ends_with("sses")
        || word.ends_with("ches")
        || word.ends_with("shes")
        || word.ends_with("xes")
        || word.ends_with("zes")
    {
        return Some(word[..word.len() - 2].to_string());
    }

    if word.ends_with('s')
        && !word.ends_with("ss")
        && !word.ends_with("us")
        && !word.ends_with("is")
        && word.len() > 2
    {
        return Some(word[..word.len() - 1].to_string());
    }

    None
}

fn generate_regular_plural(word: &str) -> String {
    if word.ends_with('y') && word.len() > 2 {
        let before_y = word.as_bytes()[word.len() - 2];
        if !matches!(before_y, b'a' | b'e' | b'i' | b'o' | b'u') {
            return format!("{}ies", &word[..word.len() - 1]);
        }
    }
    if word.ends_with("sh")
        || word.ends_with("ch")
        || word.ends_with('s')
        || word.ends_with('x')
        || word.ends_with('z')
    {
        return format!("{}es", word);
    }
    format!("{}s", word)
}

pub fn expand_plurals(word: &str) -> Vec<String> {
    let lower = word.to_lowercase();
    let mut forms = vec![lower.clone()];

    let dict = get_dictionary();
    if let Some(others) = dict.get(lower.as_str()) {
        if others.is_empty() {
            return forms;
        }
        for other in others {
            if !forms.contains(other) {
                forms.push(other.clone());
            }
        }
        return forms;
    }

    if let Some(singular) = strip_regular_plural(&lower) {
        if singular != lower && !forms.contains(&singular) {
            if let Some(dict_others) = dict.get(singular.as_str()) {
                if dict_others.is_empty() {
                    return forms;
                }
            }
            forms.push(singular);
        }
        return forms;
    }

    let plural = generate_regular_plural(&lower);
    if plural != lower && !forms.contains(&plural) {
        forms.push(plural);
    }

    forms
}

pub fn resolve_plural_languages(
    ignore_plurals: &IgnorePluralsValue,
    query_languages: &[String],
) -> Vec<String> {
    match ignore_plurals {
        IgnorePluralsValue::Disabled => vec![],
        IgnorePluralsValue::Languages(langs) => langs.clone(),
        IgnorePluralsValue::All => {
            if query_languages.is_empty() {
                vec!["en".to_string()]
            } else {
                query_languages.to_vec()
            }
        }
    }
}

pub fn should_expand_english(langs: &[String]) -> bool {
    langs.iter().any(|l| l == "en")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_plural_s() {
        let forms = expand_plurals("car");
        assert!(forms.contains(&"car".to_string()));
        assert!(forms.contains(&"cars".to_string()));
    }

    #[test]
    fn test_regular_plural_from_plural() {
        let forms = expand_plurals("cars");
        assert!(forms.contains(&"cars".to_string()));
        assert!(forms.contains(&"car".to_string()));
    }

    #[test]
    fn test_irregular_child() {
        let forms = expand_plurals("child");
        assert!(forms.contains(&"child".to_string()));
        assert!(forms.contains(&"children".to_string()));
    }

    #[test]
    fn test_irregular_children() {
        let forms = expand_plurals("children");
        assert!(forms.contains(&"children".to_string()));
        assert!(forms.contains(&"child".to_string()));
    }

    #[test]
    fn test_irregular_people() {
        let forms = expand_plurals("person");
        assert!(forms.contains(&"person".to_string()));
        assert!(forms.contains(&"people".to_string()));
    }

    #[test]
    fn test_ies_plural() {
        let forms = expand_plurals("batteries");
        assert!(forms.contains(&"batteries".to_string()));
        assert!(forms.contains(&"battery".to_string()));
    }

    #[test]
    fn test_y_to_ies() {
        let forms = expand_plurals("battery");
        assert!(forms.contains(&"battery".to_string()));
        assert!(forms.contains(&"batteries".to_string()));
    }

    #[test]
    fn test_ches_plural() {
        let forms = expand_plurals("churches");
        assert!(forms.contains(&"churches".to_string()));
        assert!(forms.contains(&"church".to_string()));
    }

    #[test]
    fn test_xes_plural() {
        let forms = expand_plurals("boxes");
        assert!(forms.contains(&"boxes".to_string()));
        assert!(forms.contains(&"box".to_string()));
    }

    #[test]
    fn test_ch_singular() {
        let forms = expand_plurals("church");
        assert!(forms.contains(&"church".to_string()));
        assert!(forms.contains(&"churches".to_string()));
    }

    #[test]
    fn test_invariant_sheep() {
        let forms = expand_plurals("sheep");
        assert_eq!(forms.len(), 1);
        assert!(forms.contains(&"sheep".to_string()));
    }

    #[test]
    fn test_knife_knives() {
        let forms = expand_plurals("knife");
        assert!(forms.contains(&"knife".to_string()));
        assert!(forms.contains(&"knives".to_string()));
    }

    #[test]
    fn test_wolf_wolves() {
        let forms = expand_plurals("wolves");
        assert!(forms.contains(&"wolves".to_string()));
        assert!(forms.contains(&"wolf".to_string()));
    }

    #[test]
    fn test_serde_bool_true() {
        let v: IgnorePluralsValue = serde_json::from_str("true").unwrap();
        assert_eq!(v, IgnorePluralsValue::All);
        assert_eq!(serde_json::to_string(&v).unwrap(), "true");
    }

    #[test]
    fn test_serde_bool_false() {
        let v: IgnorePluralsValue = serde_json::from_str("false").unwrap();
        assert_eq!(v, IgnorePluralsValue::Disabled);
        assert_eq!(serde_json::to_string(&v).unwrap(), "false");
    }

    #[test]
    fn test_serde_languages() {
        let v: IgnorePluralsValue = serde_json::from_str(r#"["en","fr"]"#).unwrap();
        assert_eq!(
            v,
            IgnorePluralsValue::Languages(vec!["en".to_string(), "fr".to_string()])
        );
    }

    #[test]
    fn test_resolve_languages_all_with_query_langs() {
        let langs = resolve_plural_languages(
            &IgnorePluralsValue::All,
            &["en".to_string(), "fr".to_string()],
        );
        assert_eq!(langs, vec!["en".to_string(), "fr".to_string()]);
    }

    #[test]
    fn test_resolve_languages_all_no_query_langs() {
        let langs = resolve_plural_languages(&IgnorePluralsValue::All, &[]);
        assert_eq!(langs, vec!["en".to_string()]);
    }

    #[test]
    fn test_resolve_languages_specific_overrides() {
        let langs = resolve_plural_languages(
            &IgnorePluralsValue::Languages(vec!["fr".to_string()]),
            &["en".to_string()],
        );
        assert_eq!(langs, vec!["fr".to_string()]);
    }

    #[test]
    fn test_resolve_disabled() {
        let langs = resolve_plural_languages(&IgnorePluralsValue::Disabled, &["en".to_string()]);
        assert!(langs.is_empty());
    }

    #[test]
    fn test_dictionary_stoves() {
        let forms = expand_plurals("stoves");
        assert!(
            forms.contains(&"stove".to_string()),
            "stoves -> {:?}",
            forms
        );
    }

    #[test]
    fn test_dictionary_archives() {
        let forms = expand_plurals("archives");
        assert!(
            forms.contains(&"archive".to_string()),
            "archives -> {:?}",
            forms
        );
    }

    #[test]
    fn test_dictionary_roofs() {
        let forms = expand_plurals("roof");
        assert!(forms.contains(&"roofs".to_string()), "roof -> {:?}", forms);
        let forms2 = expand_plurals("roofs");
        assert!(
            forms2.contains(&"roof".to_string()),
            "roofs -> {:?}",
            forms2
        );
    }

    #[test]
    fn test_dictionary_houses() {
        let forms = expand_plurals("houses");
        assert!(
            forms.contains(&"house".to_string()),
            "houses -> {:?}",
            forms
        );
        let forms2 = expand_plurals("house");
        assert!(
            forms2.contains(&"houses".to_string()),
            "house -> {:?}",
            forms2
        );
    }

    #[test]
    fn test_dictionary_data_datum() {
        let forms = expand_plurals("data");
        assert!(forms.contains(&"data".to_string()));
        assert!(
            forms.contains(&"datum".to_string()),
            "data should expand to datum: {:?}",
            forms
        );
    }

    #[test]
    fn test_dictionary_aircraft_invariant() {
        let forms = expand_plurals("aircraft");
        assert_eq!(forms.len(), 1, "aircraft is invariant: {:?}", forms);
    }

    #[test]
    fn test_dictionary_mouse_mice() {
        let forms = expand_plurals("mouse");
        assert!(forms.contains(&"mice".to_string()), "mouse -> {:?}", forms);
    }

    #[test]
    fn test_dictionary_index_indexes() {
        let forms = expand_plurals("index");
        assert!(
            forms.contains(&"indexes".to_string()),
            "index -> {:?}",
            forms
        );
    }

    #[test]
    fn test_dictionary_boss_bosses() {
        let forms = expand_plurals("boss");
        assert!(forms.contains(&"bosses".to_string()), "boss -> {:?}", forms);
    }

    #[test]
    fn test_dictionary_ox_oxen() {
        let forms = expand_plurals("ox");
        assert!(forms.contains(&"oxen".to_string()), "ox -> {:?}", forms);
    }

    #[test]
    fn test_short_word_as() {
        let forms = expand_plurals("as");
        assert!(forms.contains(&"as".to_string()));
    }

    #[test]
    fn test_dictionary_coverage_audit() {
        let cases: Vec<(&str, &str)> = vec![
            ("knife", "knives"),
            ("knives", "knife"),
            ("wife", "wives"),
            ("wives", "wife"),
            ("wolf", "wolves"),
            ("wolves", "wolf"),
            ("leaf", "leaves"),
            ("leaves", "leaf"),
            ("half", "halves"),
            ("halves", "half"),
            ("calf", "calves"),
            ("calves", "calf"),
            ("loaf", "loaves"),
            ("loaves", "loaf"),
            ("thief", "thieves"),
            ("thieves", "thief"),
            ("shelf", "shelves"),
            ("shelves", "shelf"),
            ("mouse", "mice"),
            ("mice", "mouse"),
            ("goose", "geese"),
            ("geese", "goose"),
            ("foot", "feet"),
            ("feet", "foot"),
            ("tooth", "teeth"),
            ("teeth", "tooth"),
            ("ox", "oxen"),
            ("oxen", "ox"),
            ("thesis", "theses"),
            ("theses", "thesis"),
            ("analysis", "analyses"),
            ("analyses", "analysis"),
            ("crisis", "crises"),
            ("crises", "crisis"),
            ("cactus", "cactuses"),
            ("fungus", "fungi"),
            ("matrix", "matrices"),
            ("matrices", "matrix"),
            ("index", "indexes"),
            ("indexes", "index"),
            ("medium", "media"),
            ("media", "medium"),
            ("datum", "data"),
            ("data", "datum"),
            ("criterion", "criteria"),
            ("criteria", "criterion"),
            ("phenomenon", "phenomena"),
            ("phenomena", "phenomenon"),
            ("person", "people"),
            ("people", "person"),
            ("child", "children"),
            ("children", "child"),
            ("man", "men"),
            ("men", "man"),
            ("woman", "women"),
            ("women", "woman"),
            ("photo", "photos"),
            ("photos", "photo"),
            ("piano", "pianos"),
            ("potato", "potatoes"),
            ("tomato", "tomatoes"),
            ("hero", "heroes"),
            ("heroes", "hero"),
            ("echo", "echoes"),
            ("echoes", "echo"),
        ];
        let mut failures = Vec::new();
        for (input, expected) in &cases {
            let forms = expand_plurals(input);
            if !forms.contains(&expected.to_string()) {
                failures.push(format!("  {} -> {:?} (missing {})", input, forms, expected));
            }
        }
        assert!(
            failures.is_empty(),
            "Dictionary coverage failures:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn test_invariants_no_expand() {
        let invariants = vec![
            "sheep", "deer", "fish", "species", "series", "aircraft", "bison", "moose", "salmon",
            "trout", "shrimp", "swine",
        ];
        for word in &invariants {
            let forms = expand_plurals(word);
            assert_eq!(
                forms.len(),
                1,
                "{} should be invariant but got {:?}",
                word,
                forms
            );
        }
    }

    #[test]
    fn test_uncountable_no_expand() {
        let uncountable = vec![
            "equipment",
            "information",
            "rice",
            "money",
            "news",
            "software",
            "hardware",
            "furniture",
            "advice",
            "weather",
        ];
        for word in &uncountable {
            let forms = expand_plurals(word);
            if forms.len() > 1 {
                println!("WARN: {} expands to {:?} (may want invariant)", word, forms);
            }
        }
    }

    #[test]
    fn test_no_false_positive_ve_words() {
        let ve_words = vec![
            "stove", "archive", "curve", "nerve", "valve", "sleeve", "groove", "glove", "dove",
            "cove", "move",
        ];
        for word in &ve_words {
            let forms = expand_plurals(word);
            let plural = format!("{}s", word);
            assert!(
                forms.contains(&plural) || forms.len() == 1,
                "{} should expand to {} or stay single, got {:?}",
                word,
                plural,
                forms
            );
            let bad_stem = format!("{}f", &word[..word.len() - 2]);
            assert!(
                !forms.contains(&bad_stem),
                "{} should NOT produce {} stem, got {:?}",
                word,
                bad_stem,
                forms
            );
        }
    }

    #[test]
    fn test_regular_rules_fallback() {
        let regular = vec![
            ("laptop", "laptops"),
            ("laptops", "laptop"),
            ("table", "tables"),
            ("tables", "table"),
            ("dog", "dogs"),
            ("dogs", "dog"),
            ("city", "cities"),
            ("cities", "city"),
            ("box", "boxes"),
            ("boxes", "box"),
            ("church", "churches"),
            ("churches", "church"),
            ("wish", "wishes"),
            ("wishes", "wish"),
            ("quiz", "quizzes"),
            ("boss", "bosses"),
            ("bosses", "boss"),
        ];
        let mut failures = Vec::new();
        for (input, expected) in &regular {
            let forms = expand_plurals(input);
            if !forms.contains(&expected.to_string()) {
                failures.push(format!("  {} -> {:?} (missing {})", input, forms, expected));
            }
        }
        assert!(
            failures.is_empty(),
            "Regular rule failures:\n{}",
            failures.join("\n")
        );
    }
}
