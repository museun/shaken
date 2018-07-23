pub trait CommaSeparated {
    fn comma_separate(&self) -> String;
}

macro_rules! impl_comma {
    (for $($t:ty),+) => {
        $(impl CommaSeparated for $t {
            fn comma_separate(&self) -> String {
                fn comma(n: $t, s: &mut String) {
                    if n < 1000 {
                        write!(s, "{}", n).unwrap();
                        return;
                    }
                    comma(n / 1000, s);
                    write!(s, ",{:03}", n % 1000).unwrap();
                }

                let mut buf = String::new();
                comma(*self, &mut buf);
                buf
            }
        })*
    };
}

impl_comma!(for u64, i64, usize, isize, u32, i32);
