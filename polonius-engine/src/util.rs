macro_rules! for_each_tuple {
    ($m:ident => [$($T:ident)*]) => {
        for_each_tuple!(@IMPL $m => [$($T)*]);
    };

    (@IMPL $m:ident => []) => {
        $m!();
    };

    (@IMPL $m:ident => [$H:ident $($T:ident)*]) => {
        $m!($H $($T)*);
        for_each_tuple!(@IMPL $m => [$($T)*]);
    };
}

macro_rules! count_idents {
    () => { 0 };
    ($odd:ident $($a:ident $b:ident)*) => { count_idents!($($a)*) << 1 | 1 };
    ($($a:ident $b:ident)*) => { count_idents!($($a)*) << 1 };
}

macro_rules! lg {
    ($m:path, $($tt:tt)*) => { $m!(target: "polonius_engine", $($tt)*) }
}

macro_rules! info {
    ($($tt:tt)*) => { lg!(log::info, $($tt)*) }
}
