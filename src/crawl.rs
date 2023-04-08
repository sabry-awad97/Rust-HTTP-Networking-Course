use std::collections::HashMap;

use async_recursion::async_recursion;
use reqwest::Client;
use url::Url;

use select::document::Document;
use select::predicate::Name;

#[async_recursion]
pub async fn crawl_page(
    base_url: &str,
    current_url: &str,
    mut pages: HashMap<String, usize>,
) -> HashMap<String, usize> {
    // if this is an offsite URL, bail immediately
    let current_url_obj = match Url::parse(current_url) {
        Ok(url) => url,
        Err(_) => return pages,
    };
    let base_url_obj = match Url::parse(base_url) {
        Ok(url) => url,
        Err(_) => return pages,
    };
    if current_url_obj.host_str() != base_url_obj.host_str() {
        return pages;
    }

    // normalize the URL
    let normalized_url = match normalize_url(current_url) {
        Some(url) => url,
        None => return pages,
    };

    // if we've already visited this page
    // just increase the count and don't repeat
    // the http request
    if let Some(count) = pages.get_mut(&normalized_url) {
        *count += 1;
        return pages;
    }

    // initialize this page in the map
    // since it doesn't exist yet
    pages.insert(normalized_url.clone(), 1);

    // fetch and parse the html of the currentURL
    println!("crawling {}", current_url);
    let resp = match Client::new().get(current_url).send().await {
        Ok(resp) => resp,
        Err(err) => {
            println!("{}", err);
            return pages;
        }
    };
    if resp.status().is_client_error() || resp.status().is_server_error() {
        println!("Got HTTP error, status code: {}", resp.status());
        return pages;
    }
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type.contains("text/html") {
        println!("Got non-html response: {}", content_type);
        return pages;
    }
    let html_body = match resp.text().await {
        Ok(body) => body,
        Err(err) => {
            println!("{}", err);
            return pages;
        }
    };

    let next_urls = get_urls_from_html(&html_body, base_url);
    for next_url in next_urls {
        pages = crawl_page(base_url, &next_url, pages).await;
    }

    pages
}

fn normalize_url(url: &str) -> Option<String> {
    let parsed_url = Url::parse(url).ok()?;
    let host = parsed_url.host_str()?.to_lowercase();
    let path = parsed_url.path().trim_end_matches('/').to_lowercase();
    Some(format!("{}{}", host, path))
}

fn get_urls_from_html(html: &str, base_url: &str) -> Vec<String> {
    let document = Document::from(html);
    let mut urls = Vec::new();

    for link in document.find(Name("a")).filter_map(|n| n.attr("href")) {
        match Url::parse(link) {
            Ok(url) => urls.push(url.to_string()),
            Err(_) => {
                if let Ok(base) = Url::parse(base_url) {
                    if let Ok(resolved) = base.join(link) {
                        urls.push(resolved.to_string());
                    }
                }
            }
        }
    }

    urls
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    #[test]
    fn test_normalize_url_protocol() {
        let input = "https://blog.boot.dev/path";
        let actual = normalize_url(input).unwrap();
        let expected = "blog.boot.dev/path";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_normalize_url_slash() {
        let input = "https://blog.boot.dev/path/";
        let actual = normalize_url(input).unwrap();
        let expected = "blog.boot.dev/path";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_normalize_url_capitals() {
        let input = "https://BLOG.boot.dev/path";
        let actual = normalize_url(input).unwrap();
        let expected = "blog.boot.dev/path";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_normalize_url_http() {
        let input = "http://BLOG.boot.dev/path";
        let actual = normalize_url(input).unwrap();
        let expected = "blog.boot.dev/path";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_urls_from_html_absolute() {
        let input_url = "https://blog.boot.dev";
        let input_body =
            "<html><body><a href=\"https://blog.boot.dev\"><span>Boot.dev></span></a></body></html>";
        let actual = get_urls_from_html(input_body, input_url);
        let expected = vec!["https://blog.boot.dev/"];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_urls_from_html_relative() {
        let input_url = "https://blog.boot.dev";
        let input_body =
            "<html><body><a href=\"/path/one\"><span>Boot.dev></span></a></body></html>";
        let actual = get_urls_from_html(input_body, input_url);
        let expected = vec!["https://blog.boot.dev/path/one"];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_urls_from_html_both() {
        let input_url = "https://blog.boot.dev";
        let input_body = "<html><body><a href=\"/path/one\"><span>Boot.dev></span></a><a href=\"https://other.com/path/one\"><span>Boot.dev></span></a></body></html>";
        let actual = get_urls_from_html(input_body, input_url);
        let expected = vec![
            "https://blog.boot.dev/path/one",
            "https://other.com/path/one",
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    #[ignore]
    fn test_get_urls_from_html_handle_error() {
        let input_url = "https://blog.boot.dev";
        let input_body =
            r#"<html><body><a href="path/one"><span>Boot.dev></span></a></body></html>"#;
        let actual = get_urls_from_html(input_body, input_url);
        let expected: Vec<String> = vec![];
        assert_eq!(actual, expected);
    }
}
