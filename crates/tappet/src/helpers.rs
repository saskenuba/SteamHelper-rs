use std::fmt::Display;

pub(crate) fn comma_delimited<T>(input: &[T]) -> String
where
    T: Display,
{
    let input_len = input.len();
    if input_len < 2 {
        return input[0].to_string();
    }
    let last_element = input_len - 1;
    input
        .iter()
        .enumerate()
        .take_while(|(idx, _)| *idx < last_element)
        .map(|(_, x)| x.to_string() + ",")
        .collect::<String>()
        + &input[last_element].to_string()
}

pub(crate) fn indexed_array<T>(array_name: &str, input: &[T]) -> String
where
    T: Display,
{
    input
        .iter()
        .enumerate()
        .map(|(idx, value)| format!("&{}[{}]={}", array_name, idx, value))
        .collect()
}

pub(crate) fn querify<T: ToString>(name: &str, value: T) -> String {
    "&".to_owned() + name + "=" + &value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_comma_delimited() {
        let should_be = "77777,77777";
        let input = vec!["77777", "77777"];

        let result = comma_delimited(&input);
        assert_eq!(should_be, &*result);

        let should_be = "77777";
        let input = vec!["77777"];

        let result = comma_delimited(&input);
        assert_eq!(should_be, &*result);

        let should_be = "77777,77777,77777,77777";
        let input = vec!["77777", "77777", "77777", "77777"];

        let result = comma_delimited(&input);
        assert_eq!(should_be, &*result);
    }

    #[test]
    fn t_indexed_array() {
        let should_be = "&steamid[0]=77777&steamid[1]=77777";
        let input = vec!["77777", "77777"];
        let result = indexed_array("steamid", &input);
        assert_eq!(should_be, &*result);
    }
}
