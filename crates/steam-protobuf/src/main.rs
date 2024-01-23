cfg_if::cfg_if! {
 if #[cfg(feature = "regen")] {

        mod regen;
        fn main() { regen::generate() }

    }
    else {
        fn main() { println!(r#"Enable feature "regen" in order to regenerate protobufs."#)}
    }
}
