use reqwest::{Client, Method, Url};
use scraper::{Html, Selector};
use serde::Serialize;

use crate::errors::ApiKeyError;
use crate::{mobile_request, COMMUNITY_BASE};

///! Responsible
///
async fn retrieve(client: &Client) -> Result<String, ApiKeyError> {
    let api_key_url = format!("{}{}", COMMUNITY_BASE, "/dev/apikey?l=english");
    let doc = get_html_document(client, api_key_url.as_str()).await.unwrap();
    let api = match resolve_api_key_status(doc) {
        Ok(api) => api,
        Err(err) => {
            // in this case we want to register it
            match err {
                ApiKeyError::NotRegistered => register_api_key(&client).await?,
                _ => return Err(err),
            }
        }
    };
    Ok(api)
}

/// Checks API Key state by parsing the document.
/// If key is found, returns it, otherwise, it just errors accordingly.
fn resolve_api_key_status(api_key_html: Html) -> Result<String, ApiKeyError> {
    let api_page_title_selector = Selector::parse("#mainContents > h2").unwrap();
    let api_page_key_selector = Selector::parse("#bodyContents_ex > p:nth-child(2)").unwrap();

    let title = match api_key_html.select(&api_page_title_selector).next() {
        None => return Err(ApiKeyError::GeneralError("title is blank".to_string())),
        Some(title) => title.text().collect::<String>(),
    };

    if title.contains("Validated email address required") || title.contains("Access Denied") {
        return Err(ApiKeyError::AccessDenied);
    }

    let api_key_text = match api_key_html.select(&api_page_key_selector).next() {
        None => return Err(ApiKeyError::GeneralError("key node is blank".to_string())),
        Some(api_key_text) => api_key_text.text().collect::<String>(),
    };

    if api_key_text.contains("Registering for a Steam Web API Key") {
        return Err(ApiKeyError::NotRegistered);
    }

    let key_vec = api_key_text.split("Key: ").collect::<Vec<&str>>();
    let api_key = key_vec[1];

    if api_key.len() != 32 {
        return Err(ApiKeyError::GeneralError(format!("Size should be 32. Found: {}", api_key)));
    }

    Ok(api_key.to_string())
}

/// Request access to an API Key
/// The account should be validated before.
async fn register_api_key(client: &Client) -> Result<String, ApiKeyError> {
    let api_register_url = format!("{}{}", COMMUNITY_BASE, "/dev/registerkey");
    let register_request = ApiKeyRegisterRequest::default();

    let response =
        mobile_request(client, &api_register_url, Method::POST, None, Some(register_request)).await;

    Ok("".to_string())
}

#[derive(Debug, Serialize)]
struct ApiKeyRegisterRequest<'a> {
    #[serde(rename = "agreeToTerms")]
    agree_to_terms: &'a str,
    domain: &'a str,
    #[serde(rename = "Submit")]
    submit: &'a str,
}

impl<'a> Default for ApiKeyRegisterRequest<'a> {
    fn default() -> Self {
        Self { agree_to_terms: "agreed", domain: "localhost", submit: "Register" }
    }
}

async fn get_html_document(client: &Client, url: &str) -> Result<Html, reqwest::Error> {
    let parsed_url = Url::parse(url).unwrap();
    let response = client.get(parsed_url).send().await?;
    let response_text = response.text().await?;

    let _html_document = Html::parse_document(&response_text);
    Ok(_html_document)
}

#[cfg(test)]
mod tests {
    use scraper::Selector;

    use super::*;

    fn client() -> Client {
        reqwest::Client::new()
    }

    #[test]
    fn test_resolve_api_key_status() {
        let api_doc = Html::parse_document(include_str!("../assets/api_ok.html"));
        let api = resolve_api_key_status(api_doc).unwrap();
        assert_eq!(api, "D805666DF5E380C5F8A89B8F8A0814B8");
    }

    #[tokio::test]
    async fn get_html() {
        let document =
            get_html_document(&client(), "https://www.york.ac.uk/teaching/cws/wws/webpage1.html")
                .await
                .unwrap();
        let title_selector = Selector::parse("title").unwrap();
        let title = document.select(&title_selector).next().unwrap().text();
        assert_eq!(title.collect::<String>(), "webpage1");
    }
}
