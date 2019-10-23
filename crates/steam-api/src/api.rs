use tokio::future::Future;

const STEAM_API_BASE_ADDRESS: &str = "https://api.steampowered.com/";

type Param<'a> = (&'a str, &'a str);

pub struct APIBuilder {
    request_link: String,
}

impl APIBuilder {
    pub fn new(
        service: &str,
        method: &str,
        api_key: &str,
        params: Option<Vec<Param>>,
    ) -> APIBuilder {
        let key = format!("?key={}", api_key);
        let parameters: String = APIBuilder::param_builder(params);

        let whole_address =
            format!("{}{}/{}/v1/{}{}", STEAM_API_BASE_ADDRESS, service, method, key, parameters);

        APIBuilder { request_link: whole_address }
    }

    pub fn setup(&self) -> impl Future + '_ {
        reqwest::get(&self.request_link)
    }

    pub fn dump_request_link(&self) -> &str {
        &self.request_link
    }

    fn param_builder(params: Option<Vec<Param>>) -> String {
        let mut params_stringify = String::new();
        params.unwrap().iter().for_each(|t| {
            params_stringify.push('&');
            params_stringify.push_str(&t.0);
            params_stringify.push('=');
            params_stringify.push_str(&t.1);
        });

        params_stringify
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn build_api_link() {
        let mut vector = Vec::<(&str, &str)>::new();
        vector.push(("cellid", "25"));

        let api_call = APIBuilder::new("ISteamDirectory", "GetCMList", "1", Option::from(vector));

        assert_eq!(
            "https://api.steampowered.com/ISteamDirectory/GetCMList/v1/?key=1&cellid=25",
            api_call.request_link
        )
    }
}
