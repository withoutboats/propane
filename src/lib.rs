#![feature(generator_trait, try_trait, generators)]

#[doc(hidden)]
pub mod __internal {
    use std::marker::Unpin;
    use std::ops::{Generator, GeneratorState};
    use std::pin::Pin;

    pub struct GenIter<G>(pub G);

    impl<G: Generator<Return = ()> + Unpin> Iterator for GenIter<G> {
        type Item = G::Yield;

        fn next(&mut self) -> Option<Self::Item> {
            match Pin::new(&mut self.0).resume(()) {
                GeneratorState::Yielded(item)   => Some(item),
                GeneratorState::Complete(())    => None,
            }
        }
    }

    #[macro_export]
    macro_rules! gen_try {
        ($e:expr) => {{
            use std::ops::Try;
            match Try::into_result($e) {
                Ok(ok)      => ok,
                Err(err)    => {
                    yield <_ as Try>::from_error(err);
                    return;
                }
            }
        }}
    }
}
