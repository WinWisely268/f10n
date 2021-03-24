extern crate google_translate3 as translate3;

use anyhow::{Result};
use translate3::{TranslateTextRequest, Translate, DetectLanguageRequest, DetectedLanguage};
use yup_oauth2::{service_account_key_from_file, ServiceAccountAccess};
use clap::{App, Arg};
use std::collections::HashMap;
use serde_json::Value;
use hashlink::LinkedHashMap;
use std::io::Write;
use hyper::{Client, net::HttpsConnector};
use hyper_rustls::{TlsClient};

struct Cache {
    db: sled::Db,
}

impl Cache {
    fn new(cache_path: &str) -> Result<Self> {
        // Create local storage
        let c = sled::open(cache_path)?;
        Ok(Cache {
            db: c,
        })
    }

    fn find_cache_content(&self, key: &str, lang: &str) -> Result<String> {
        let res = self.db.get(format!("{}_{}", lang, key))?;
        match res {
            Some(v) => {
                let vecv = v.to_vec();
                let result_str = std::str::from_utf8(&*vecv)?;
                Ok(result_str.to_string())
            }
            None => Ok("".to_string())
        }
    }

    fn add_to_cache(&self, key: &str, lang: &str, value: &str) -> Result<()> {
        let res = self.find_cache_content(key, lang)?;
        if !res.is_empty() {
            return Ok(());
        }
        self.db.insert(format!("{}_{}", lang, key).as_str(), value)?;

        Ok(())
    }

    fn get_untranslated_translated(&self, lang: &str, keys: Vec<String>) -> Result<(Vec<String>, HashMap<String, String>)> {
        let mut untranslated: Vec<String> = vec!();
        let mut translated: HashMap<String, String> = HashMap::new();
        for k in keys {
            let res = self.find_cache_content(&k, lang)?;
            if res.is_empty() {
                untranslated.push(k);
                continue;
            }
            translated.insert(k, res);
        }
        Ok((untranslated, translated))
    }
}

#[cfg(test)]
mod test_cache {
    use super::Cache;
    use super::Result;


    #[test]
    fn test_insert_cache() -> Result<()> {
        let c = Cache::new("./testdata.db")?;
        c.add_to_cache("hello", "en", "hello")?;
        c.add_to_cache("hello", "es", "hola")?;

        assert_eq!("hola".to_string(), c.find_cache_content("hello", "es")?);
        Ok(())
    }

    #[test]
    fn test_get_untrans() -> Result<()> {
        let c = Cache::new("./testdata.db")?;
        c.add_to_cache("hello", "en", "hello")?;
        c.add_to_cache("hello", "es", "hola")?;

        let res = c.get_untranslated_translated("fr", vec!("hello".to_string()))?;
        assert_eq!(1, res.0.len());
        Ok(())
    }
}

struct Svc {
    client: Translate<Client, ServiceAccountAccess<Client>>,
    project: String,
}

impl Svc {
    fn new(secret_path: &str) -> Result<Self> {
        let sec = service_account_key_from_file(&secret_path.to_string())?;
        let project = sec.clone().project_id.unwrap();
        let client = Client::with_connector(HttpsConnector::new(TlsClient::new()));
        let acc = ServiceAccountAccess::new(sec, client);
        let client = Translate::new(
            Client::with_connector(
                HttpsConnector::new(TlsClient::new())
            ), acc,
        );
        Ok(Svc {
            client,
            project: format!("projects/{}", project),
        })
    }
    fn detect_lang(&self, input: &str) -> Result<String> {
        let req = DetectLanguageRequest {
            content: Some(input.to_string()),
            mime_type: Some("text/plain".to_string()),
            model: None,
            labels: None,
        };
        let res = self.client.projects()
            .detect_language(req, &*self.project).doit();
        match res {
            Ok(r) => {
                let lang = r.1.languages.unwrap_or_default()
                    .first()
                    .unwrap_or(&DetectedLanguage { language_code: None, confidence: None })
                    .language_code.clone().unwrap_or_default();
                Ok(lang)
            }
            Err(e) => Err(anyhow::Error::msg(e.to_string())),
        }
    }

    fn translate(&self, inputs: Vec<String>, to_lang: &str) -> Result<Vec<String>> {
        let first_word = inputs.first().unwrap();
        let from_lang = self.detect_lang(first_word)?;
        if from_lang == to_lang {
            return Ok(inputs);
        }
        let req = TranslateTextRequest {
            mime_type: Some("text/plain".to_string()),
            source_language_code: Some(from_lang),
            target_language_code: Some(to_lang.to_string()),
            glossary_config: None,
            model: None,
            labels: None,
            contents: Some(inputs),
        };

        let res = self.client.projects()
            .translate_text(req, &*self.project).doit();
        match res {
            Err(e) => Err(anyhow::Error::msg(e.to_string())),
            Ok(res) => {
                Ok(res.1.translations.iter()
                    .map(|t| {
                        let mut tled: Vec<String> = vec!();
                        for x in t {
                            let tl = x.clone().translated_text.unwrap();
                            tled.push(tl);
                        }
                        tled
                    })
                    .flatten()
                    .collect::<Vec<String>>())
            }
        }
    }
}

#[cfg(test)]
mod test_translate {
    use super::{Svc, Result};

    #[test]
    fn detect_lang() -> Result<()> {
        let client_secret_path = &std::env::var("GOOGLE_APPLICATION_CREDENTIALS")?;
        let s = Svc::new(client_secret_path)?;
        let v = s.detect_lang("Country Road")?;
        assert_eq!("en".to_string(), v);

        let v = s.detect_lang("Keine Artikel verfügbar")?;
        assert_eq!("de".to_string(), v);
        Ok(())
    }

    #[test]
    fn translate() -> Result<()> {
        let client_secret_path = &std::env::var("GOOGLE_APPLICATION_CREDENTIALS")?;
        let s = Svc::new(client_secret_path)?;
        let inputs = vec!("Search", "No Items available", "Buffalo Soldier", "Soldier of Fortune").into_iter().map(|x| x.to_string()).collect::<Vec<String>>();
        let result = s.translate(inputs, "de")?;

        assert_ne!(vec!("emp"), result);
        assert_eq!(vec!("Suche", "Keine Artikel verfügbar", "Büffelsoldat", "Glückssoldat").into_iter().map(|x| x.to_string()).collect::<Vec<String>>(), result);
        println!("{:?}", result);

        Ok(())
    }
}


fn main() -> Result<()> {
    let matches = App::new("f10n")
        .version("0.1")
        .author("Alexander Adhyatma <alex@asiatech.dev>")
        .about("translate template arb file to any language for flutter")
        .args(
            &[
                Arg::with_name("target-languages")
                    .help("target languages separated by space i.e. fr de es")
                    .long("lang")
                    .short("l")
                    .multiple(true)
                    .min_values(1)
                    .default_value("fr de es it tr")
                    .required(false),
                Arg::with_name("cache")
                    .help("path to the cache file (json)")
                    .long("cache")
                    .short("c")
                    .takes_value(true)
                    .default_value("translations_cache.json"),
                Arg::with_name("arb-template")
                    .help("path to the arb template")
                    .long("arb-template")
                    .short("t")
                    .takes_value(true)
                    .default_value("app_en.arb"),
                Arg::with_name("prefix-dir")
                    .help("prefix-dir for the output")
                    .long("output")
                    .short("o")
                    .takes_value(true)
                    .default_value("./testdata")
            ])
        .get_matches();

    let cache_arg = matches.value_of("cache").unwrap(); // config file is required anyway
    let langs = matches.values_of("target-languages").unwrap().collect::<Vec<&str>>();
    let arb_template = matches.value_of("arb-template").unwrap();
    let prefix = matches.value_of("prefix-dir").unwrap();

    std::fs::create_dir_all(prefix)?;

    // cache
    let cache = Cache::new(cache_arg)?;

    // linked hash map
    let linked_arb = lhm_from_template(arb_template)?;
    let to_be_tled = to_be_translated_words(&linked_arb);

    let client_secret_path = &std::env::var("GOOGLE_APPLICATION_CREDENTIALS")?;
    let s = Svc::new(client_secret_path)?;
    let translations = translate_all(&cache, &s, langs, to_be_tled)?;
    create_l10n_files(&translations, &linked_arb, prefix)?;

    Ok(())
}

fn lhm_from_template(arb_template_path: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
    let arb = std::fs::read(arb_template_path)?;
    let arb_val: serde_json::Value = serde_json::from_slice(&arb).unwrap();
    let arb_obj: serde_json::Map<String, serde_json::Value> = arb_val.as_object().unwrap().clone();
    Ok(arb_obj)
}

fn to_be_translated_words(m: &serde_json::Map<String, Value>) -> Vec<String> {
    m.into_iter()
        .filter(|(k, _)| !k.starts_with('@'))
        .map(|(_, v)| v.as_str().unwrap().to_string())
        .collect::<Vec<String>>()
}

fn translate_all(c: &Cache, s: &Svc, langs: Vec<&str>, words: Vec<String>) -> Result<LinkedHashMap<String, LinkedHashMap<String, String>>> {
    let mut tl: LinkedHashMap<String, LinkedHashMap<String, String>> = LinkedHashMap::new();
    for lang in langs {
        let mut translations: LinkedHashMap<String, String> = LinkedHashMap::new();
        // check cache for translations and untranslated records
        let w = words.clone();
        let (untranslated, translated) = c.get_untranslated_translated(lang, w)?;
        translated.into_iter()
            .for_each(|(k, v)| {
                translations.insert(k, v);
            });
        if !untranslated.is_empty() {
            let result = s.translate(untranslated.clone(), lang)?;
            for i in 0..untranslated.len() {
                let untr = &untranslated[i].clone();
                let res = &result[i].clone();
                translations.insert(untr.to_string(), res.to_string());
                c.add_to_cache(untr, lang, res)?;
            }
        }
        tl.insert(lang.to_string(), translations);
    }
    Ok(tl)
}

fn create_l10n_files(
    translations: &LinkedHashMap<String, LinkedHashMap<String, String>>,
    orig: &serde_json::Map<String, serde_json::Value>,
    prefix_dir: &str,
) -> Result<()> {
    for (lang, v) in translations {
        let mut tl: serde_json::Map<String, Value> = serde_json::Map::new();
        for (orig_key, orig_val) in orig {
            if orig_val.is_string() && !orig_key.starts_with('@') {
                let ovstring = serde_json::to_string(orig_val)?.replace("\"", "");
                let translated_val = v.get(&ovstring).unwrap().clone();
                tl.insert(orig_key.to_string(), serde_json::Value::from(translated_val));
            } else {
                tl.insert(orig_key.to_string(), orig_val.to_owned());
            }
        }
        let json_content = serde_json::to_string_pretty(&tl)?;
        let fname = format!("app_{}.arb", lang);

        let p = prefix_dir.to_string().clone();
        let file_name = std::path::Path::new(&p).join(fname);

        let mut f = std::fs::File::create(file_name)?;
        f.write_all(json_content.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod test_arb {
    use super::*;

    #[test]
    fn test_arb() -> Result<()> {
        let linked_arb = lhm_from_template("./intl_messages.arb")?;
        assert_ne!(0, linked_arb.len());
        Ok(())
    }

    #[test]
    fn test_create_l10n_files() -> Result<()> {
        let c = Cache::new("./testdata.db")?;
        let client_secret_path = &std::env::var("GOOGLE_APPLICATION_CREDENTIALS")?;
        let s = Svc::new(client_secret_path)?;
        let linked_arb = lhm_from_template("./intl_messages.arb")?;
        let to_be_tled = to_be_translated_words(&linked_arb)?;
        let langs = vec!("en",
                         "fr",
                         "de",
        );
        let translations = translate_all(&c, &s, langs, to_be_tled)?;
        let res = create_l10n_files(&translations, &linked_arb, "./testdata");
        Ok(())
    }
}
