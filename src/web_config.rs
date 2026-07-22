use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use anyhow::{Context, Result, anyhow, ensure};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3000;

/// Identifies where a web configuration setting came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigSource {
    Environment,
    Dotenv,
    Default,
}

/// Runtime settings for the web listener and its single outbound proxy.
#[derive(Clone)]
pub struct WebConfig {
    pub host: String,
    pub port: u16,
    pub proxy_url: Option<reqwest::Url>,
    pub host_source: ConfigSource,
    pub port_source: ConfigSource,
    pub proxy_source: ConfigSource,
}

impl WebConfig {
    /// Loads settings using process environment, current-directory `.env`, then defaults.
    pub fn load() -> Result<Self> {
        let current_dir =
            env::current_dir().context("failed to determine the current directory")?;
        let dotenv = read_optional_dotenv(&current_dir.join(".env"))?;

        Self::from_sources(
            |name| match env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(env::VarError::NotPresent) => Ok(None),
                Err(env::VarError::NotUnicode(_)) => {
                    Err(anyhow!("environment variable {name} is not valid Unicode"))
                }
            },
            dotenv.as_deref(),
        )
    }

    /// Returns a listener address suitable for later DNS resolution or binding.
    pub fn socket_address(&self) -> String {
        socket_address(&self.host, self.port)
    }

    /// Returns `direct` or a proxy URL with credentials and URL details removed.
    pub fn safe_proxy_display(&self) -> String {
        self.proxy_url
            .as_ref()
            .map(safe_proxy_url)
            .unwrap_or_else(|| "direct".to_string())
    }

    /// Builds the configured single proxy, if any.
    pub fn proxy(&self) -> Result<Option<reqwest::Proxy>> {
        self.proxy_url
            .clone()
            .map(reqwest::Proxy::all)
            .transpose()
            .context("validated PROXY_URL was rejected by reqwest")
    }

    /// Disables reqwest's standard environment proxies and applies only `PROXY_URL`.
    pub fn apply_proxy(&self, builder: reqwest::ClientBuilder) -> Result<reqwest::ClientBuilder> {
        let builder = builder.no_proxy();
        Ok(match self.proxy()? {
            Some(proxy) => builder.proxy(proxy),
            None => builder,
        })
    }

    fn from_sources<F>(mut environment: F, dotenv_content: Option<&str>) -> Result<Self>
    where
        F: FnMut(&str) -> Result<Option<String>>,
    {
        let dotenv = parse_dotenv(dotenv_content.unwrap_or_default())?;

        let (host, host_source) = nonblank_setting(
            environment("HOST")?,
            dotenv.get("HOST").map(String::as_str),
            DEFAULT_HOST,
        );
        let (port, port_source) = nonblank_setting(
            environment("PORT")?,
            dotenv.get("PORT").map(String::as_str),
            &DEFAULT_PORT.to_string(),
        );
        let (proxy, proxy_source) = proxy_setting(
            environment("PROXY_URL")?,
            dotenv.get("PROXY_URL").map(String::as_str),
        );

        let port = parse_port(&port)
            .with_context(|| format!("invalid PORT from {}", source_name(port_source)))?;
        validate_host(&host, port)
            .with_context(|| format!("invalid HOST from {}", source_name(host_source)))?;
        let proxy_url = proxy
            .as_deref()
            .map(parse_proxy_url)
            .transpose()
            .with_context(|| format!("invalid PROXY_URL from {}", source_name(proxy_source)))?;

        Ok(Self {
            host,
            port,
            proxy_url,
            host_source,
            port_source,
            proxy_source,
        })
    }
}

impl fmt::Debug for WebConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("proxy_url", &self.safe_proxy_display())
            .field("host_source", &self.host_source)
            .field("port_source", &self.port_source)
            .field("proxy_source", &self.proxy_source)
            .finish()
    }
}

fn read_optional_dotenv(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn nonblank_setting(
    environment: Option<String>,
    dotenv: Option<&str>,
    default: &str,
) -> (String, ConfigSource) {
    if let Some(value) = environment.filter(|value| !value.trim().is_empty()) {
        return (value.trim().to_string(), ConfigSource::Environment);
    }
    if let Some(value) = dotenv.filter(|value| !value.trim().is_empty()) {
        return (value.trim().to_string(), ConfigSource::Dotenv);
    }
    (default.to_string(), ConfigSource::Default)
}

fn proxy_setting(
    environment: Option<String>,
    dotenv: Option<&str>,
) -> (Option<String>, ConfigSource) {
    if let Some(value) = environment {
        let value = value.trim();
        return (
            (!value.is_empty()).then(|| value.to_string()),
            ConfigSource::Environment,
        );
    }
    if let Some(value) = dotenv {
        let value = value.trim();
        return (
            (!value.is_empty()).then(|| value.to_string()),
            ConfigSource::Dotenv,
        );
    }
    (None, ConfigSource::Default)
}

fn parse_port(value: &str) -> Result<u16> {
    let port = value
        .parse::<u16>()
        .with_context(|| format!("{value:?} is not an unsigned 16-bit integer"))?;
    ensure!(port != 0, "port must be nonzero");
    Ok(port)
}

fn validate_host(host: &str, port: u16) -> Result<()> {
    ensure!(!host.is_empty(), "host must not be empty");

    let address = socket_address(host, port);
    if address.parse::<SocketAddr>().is_ok() {
        return Ok(());
    }

    ensure!(
        !host.chars().any(char::is_whitespace),
        "host must not contain whitespace"
    );
    ensure!(
        !host.contains(['/', '?', '#', '@', ':', '[', ']']),
        "host must be an IP address or hostname without a port"
    );

    let url = reqwest::Url::parse(&format!("http://{address}/"))
        .context("host cannot form a resolution-ready socket address")?;
    ensure!(url.host().is_some(), "host is missing");
    ensure!(
        url.port_or_known_default() == Some(port),
        "host contains an invalid authority"
    );
    Ok(())
}

fn socket_address(host: &str, port: u16) -> String {
    let host = host.trim();
    if host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .is_some_and(|value| value.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()))
    {
        format!("{host}:{port}")
    } else if host.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn parse_proxy_url(value: &str) -> Result<reqwest::Url> {
    let (_, authority) = value
        .split_once("://")
        .context("must include an explicit http:// or https:// scheme")?;
    ensure!(
        !authority.is_empty() && !authority.starts_with('/'),
        "URL must include a host"
    );
    let url = reqwest::Url::parse(value).context("must be a valid URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "scheme must be http or https"
    );
    ensure!(url.host().is_some(), "URL must include a host");
    reqwest::Proxy::all(url.clone()).context("URL is not accepted by reqwest as a proxy")?;
    Ok(url)
}

fn safe_proxy_url(url: &reqwest::Url) -> String {
    let credentials = if url.username().is_empty() && url.password().is_none() {
        ""
    } else {
        "***:***@"
    };
    let host = match url.host_str() {
        Some(host) if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) => {
            format!("[{host}]")
        }
        Some(host) => host.to_string(),
        None => return "invalid proxy".to_string(),
    };
    let port = url
        .port()
        .map_or_else(String::new, |port| format!(":{port}"));

    format!("{}://{credentials}{host}{port}", url.scheme())
}

fn source_name(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::Environment => "environment",
        ConfigSource::Dotenv => ".env",
        ConfigSource::Default => "defaults",
    }
}

fn parse_dotenv(content: &str) -> Result<HashMap<String, String>> {
    let mut values = HashMap::new();

    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = strip_export(line);
        let Some((key, raw_value)) = line.split_once('=') else {
            let key = line.split_whitespace().next().unwrap_or_default();
            ensure!(
                !matches!(key, "HOST" | "PORT" | "PROXY_URL"),
                "invalid .env assignment for {key} on line {}",
                index + 1
            );
            continue;
        };
        let key = key.trim();
        if !matches!(key, "HOST" | "PORT" | "PROXY_URL") {
            continue;
        }

        let value = parse_dotenv_value(raw_value)
            .with_context(|| format!("invalid .env value for {key} on line {}", index + 1))?;
        values.insert(key.to_string(), value);
    }

    Ok(values)
}

fn strip_export(line: &str) -> &str {
    let Some(rest) = line.strip_prefix("export") else {
        return line;
    };
    if rest.starts_with(char::is_whitespace) {
        rest.trim_start()
    } else {
        line
    }
}

fn parse_dotenv_value(raw_value: &str) -> Result<String> {
    let value = raw_value.trim();
    let Some(quote) = value
        .chars()
        .next()
        .filter(|character| matches!(character, '\'' | '"'))
    else {
        let comment = value
            .char_indices()
            .find(|(index, character)| {
                *character == '#'
                    && value[..*index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace)
            })
            .map_or(value.len(), |(index, _)| index);
        return Ok(value[..comment].trim_end().to_string());
    };

    let mut parsed = String::new();
    let mut escaped = false;
    let mut closing_index = None;
    for (index, character) in value[quote.len_utf8()..].char_indices() {
        if quote == '"' && escaped {
            parsed.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if quote == '"' && character == '\\' {
            escaped = true;
        } else if character == quote {
            closing_index = Some(quote.len_utf8() + index + character.len_utf8());
            break;
        } else {
            parsed.push(character);
        }
    }

    ensure!(!escaped, "quoted value ends with an escape");
    let closing_index = closing_index.context("quoted value is not terminated")?;
    let remainder = value[closing_index..].trim();
    ensure!(
        remainder.is_empty() || remainder.starts_with('#'),
        "unexpected content after quoted value"
    );
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(environment: &[(&str, &str)], dotenv: Option<&str>) -> Result<WebConfig> {
        WebConfig::from_sources(
            |name| {
                Ok(environment
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_string()))
            },
            dotenv,
        )
    }

    #[test]
    fn uses_defaults_when_sources_are_absent() {
        let config = config(&[], None).unwrap();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(config.proxy_url.is_none());
        assert_eq!(config.host_source, ConfigSource::Default);
        assert_eq!(config.port_source, ConfigSource::Default);
        assert_eq!(config.proxy_source, ConfigSource::Default);
        assert_eq!(config.socket_address(), "127.0.0.1:3000");
        assert_eq!(config.safe_proxy_display(), "direct");
    }

    #[test]
    fn environment_values_override_dotenv_values() {
        let config = config(
            &[
                ("HOST", "env.example.com"),
                ("PORT", "4100"),
                ("PROXY_URL", "https://env-proxy.example:8443"),
            ],
            Some("HOST=dotenv.example.com\nPORT=4200\nPROXY_URL=http://dotenv-proxy.example:8080"),
        )
        .unwrap();

        assert_eq!(config.host, "env.example.com");
        assert_eq!(config.port, 4100);
        assert_eq!(
            config.proxy_url.as_ref().unwrap().as_str(),
            "https://env-proxy.example:8443/"
        );
        assert_eq!(config.host_source, ConfigSource::Environment);
        assert_eq!(config.port_source, ConfigSource::Environment);
        assert_eq!(config.proxy_source, ConfigSource::Environment);
    }

    #[test]
    fn blank_host_and_port_fall_back_but_blank_environment_proxy_forces_direct() {
        let config = config(
            &[("HOST", "  "), ("PORT", ""), ("PROXY_URL", "  ")],
            Some("HOST=dotenv.example.com\nPORT=4200\nPROXY_URL=http://proxy.example:8080"),
        )
        .unwrap();

        assert_eq!(config.host, "dotenv.example.com");
        assert_eq!(config.port, 4200);
        assert!(config.proxy_url.is_none());
        assert_eq!(config.host_source, ConfigSource::Dotenv);
        assert_eq!(config.port_source, ConfigSource::Dotenv);
        assert_eq!(config.proxy_source, ConfigSource::Environment);
    }

    #[test]
    fn dotenv_values_override_defaults_and_support_quotes_and_export() {
        let config = config(
            &[],
            Some(
                "export HOST='localhost' # listener\nPORT=\"5000\"\nPROXY_URL=https://proxy.example:4443 # outbound",
            ),
        )
        .unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5000);
        assert_eq!(config.proxy_source, ConfigSource::Dotenv);
        assert_eq!(config.safe_proxy_display(), "https://proxy.example:4443");
    }

    #[test]
    fn rejects_malformed_known_dotenv_assignments() {
        let error = config(&[], Some("HOST localhost\n")).unwrap_err();

        assert!(format!("{error:#}").contains("invalid .env assignment for HOST on line 1"));
    }

    #[test]
    fn rejects_zero_and_out_of_range_ports() {
        let zero = config(&[("PORT", "0")], None).unwrap_err();
        let too_large = config(&[("PORT", "65536")], None).unwrap_err();

        assert!(format!("{zero:#}").contains("port must be nonzero"));
        assert!(format!("{too_large:#}").contains("unsigned 16-bit integer"));
    }

    #[test]
    fn accepts_hostname_ipv4_and_ipv6_socket_addresses() {
        let hostname = config(&[("HOST", "service.internal")], None).unwrap();
        let ipv4 = config(&[("HOST", "0.0.0.0")], None).unwrap();
        let ipv6 = config(&[("HOST", "::1")], None).unwrap();
        let bracketed_ipv6 = config(&[("HOST", "[::1]")], None).unwrap();
        let default_url_port = config(&[("HOST", "localhost"), ("PORT", "80")], None).unwrap();

        assert_eq!(hostname.socket_address(), "service.internal:3000");
        assert_eq!(ipv4.socket_address(), "0.0.0.0:3000");
        assert_eq!(ipv6.socket_address(), "[::1]:3000");
        assert_eq!(bracketed_ipv6.socket_address(), "[::1]:3000");
        assert_eq!(default_url_port.socket_address(), "localhost:80");
    }

    #[test]
    fn rejects_hosts_with_ports_or_url_components() {
        for host in [
            "localhost:4000",
            "http://localhost",
            "host/path",
            "user@host",
        ] {
            let error = config(&[("HOST", host)], None).unwrap_err();
            assert!(format!("{error:#}").contains("invalid HOST"));
        }
    }

    #[test]
    fn rejects_non_http_proxy_urls_and_urls_without_hosts() {
        for proxy in [
            "socks5://proxy.example:1080",
            "http:///path",
            "proxy.example:8080",
        ] {
            let error = config(&[("PROXY_URL", proxy)], None).unwrap_err();
            assert!(format!("{error:#}").contains("invalid PROXY_URL"));
        }
    }

    #[test]
    fn safe_proxy_display_redacts_credentials_path_query_and_fragment() {
        let config = config(
            &[(
                "PROXY_URL",
                "http://alice:secret@127.0.0.1:7890/private?token=secret#fragment",
            )],
            None,
        )
        .unwrap();

        assert_eq!(config.safe_proxy_display(), "http://***:***@127.0.0.1:7890");
        let debug = format!("{config:?}");
        assert!(!debug.contains("alice"));
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("private"));
    }
}
