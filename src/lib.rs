//! This library provides a generator macro, to define generators.
//!
//! It is intended to explore one particular point in the design space of generators. More
//! documentation can be found in the description of the attribute.
#![feature(generator_trait)]

/// This macro can be applied to functions to make them into generators.
///
/// Functions annotated with this attribute use the `yield` keyword, instead of the `return`
/// keyword. They yield an item and then continue. When you call a generator function, you get an
/// iterator of the type it yields, rather than just one value of that type.
///
/// You can still use the `return` keyword to terminate the generator early, but the `return`
/// keyword cannot take a value; it only terminates the function.
///
/// The behavior of `?` is also modified in these functions. In the event of an error, the
/// generator yields the error value, and then the next time it is resumed it returns `None`.
///
/// ## Forbidding self-references
///
/// Unlike async functions, generators cannot contain self-references: a reference into their stack
/// space that exists across a yield point. Instead, anything you wish to have by reference you
/// should move out of the state of the generator, taking it as an argument, or else not holding it
/// by reference across a point that you yield.
///
/// ## Unstable features
///
/// In order to use this attribute, you must turn on all of these features:
/// - `generators`
/// - `generator_trait`
/// - `try_trait`
///
/// ## Example
///
/// ```rust
/// #![feature(generators, generator_trait, try_trait)]
///
/// #[propane::generator]
/// fn fizz_buzz() -> String {
///    for x in 1..101 {
///       match (x % 3 == 0, x % 5 == 0) {
///           (true, true)  => yield String::from("FizzBuzz"),
///           (true, false) => yield String::from("Fizz"),
///           (false, true) => yield String::from("Buzz"),
///           (..)          => yield x.to_string(),
///       }
///    }
/// }
///
/// fn main() {
///     let mut fizz_buzz = fizz_buzz();
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "1");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "2");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Fizz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "4");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Buzz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Fizz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "7");
///
///     // yada yada yada
///     let mut fizz_buzz = fizz_buzz.skip(90);
///
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "98");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Fizz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Buzz");
///     assert!(fizz_buzz.next().is_none());
/// }
/// ```
pub use propane_macros::generator;

#[doc(hidden)]
pub mod __internal {
    use std::marker::Unpin;
    use std::ops::{Generator, GeneratorState};
    use std::pin::Pin;
    use std::task::{Context, Poll};

    pub use futures_core::Stream;

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

    pub struct GenStream<G>(G);

    impl<G> GenStream<G> {
        pub unsafe fn new(gen: G) -> GenStream<G> { GenStream(gen) }
    }

    impl<G: Generator<*mut (), Yield = Poll<T>, Return = ()>, T> Stream for GenStream<G> {
        type Item = T;

        fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let ctx: *mut () = ctx as *mut Context<'_> as *mut ();

            unsafe {
                let gen: Pin<&mut G> = Pin::map_unchecked_mut(self, |this| &mut this.0);

                match gen.resume(ctx) {
                    GeneratorState::Complete(())                => Poll::Ready(None),
                    GeneratorState::Yielded(Poll::Ready(item))  => Poll::Ready(Some(item)),
                    GeneratorState::Yielded(Poll::Pending)      => Poll::Pending,
                }
            }
        }
    }

    #[doc(hidden)]
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

    #[doc(hidden)]
    #[macro_export]
    macro_rules! async_gen_try {
        ($e:expr) => {{
            use std::ops::Try;
            match Try::into_result($e) {
                Ok(ok)      => ok,
                Err(err)    => {
                    yield std::task::Poll::Ready(<_ as Try>::from_error(err));
                    return;
                }
            }
        }}
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! async_gen_yield {
        ($e:expr) => {{
            yield std::task::Poll::Ready($e)
        }}
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! async_gen_await {
        ($e:expr, $ctx:expr) => {{
            unsafe {
                use std::pin::Pin;
                use std::task::{Poll, Context};
                let ctx = &mut *($ctx as *mut Context<'_>);
                let mut e = $e;
                let mut future = Pin::new_unchecked(&mut e);
                loop {
                    match std::future::Future::poll(Pin::as_mut(&mut future), ctx) {
                        Poll::Ready(x)   => break x,
                        Poll::Pending    => $ctx = yield Poll::Pending,
                    }
                }
            }
        }}
    }
}
