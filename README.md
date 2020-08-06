# propane - Rust generators

Propane is a thin wrapper around the unstable generator feature, allowing users to create free
functions as generators. It implements a particular idea for how generators should be designed; a
big part of why generators are still unstalbe today is that these design questions are still
undecided.

The syntax looks like this:

```rust
#[propane::generator]
fn foo() -> i32 {
    for n in 0i32..10 {
        yield n;
    }
}
```

Because it is a macro, it does not work as well as a native language feature would, and has worse
error messages.

## Design decisions

Propane is designed to allow users to write generators for the purpose of implementing iterators.
For that reason, its generators are restricted in some important ways. These are the intentional
design restrictions of propane (that is, these are not limitations because of bugs, they are not
intended to be lifted):

1. A propane generator becomes a function that returns an `impl Iterator`; the iterator interface is
   the only interface users can use with the generator's return type.
2. A propane generator can only return `()`, it cannot yield one type and then return another
   interesting type. The `?` operator yields the error and then, on the next resumption, returns.
3. A propane generator implements Unpin, and cannot be self-referential (unlike async functions).

In essence propane allows the users

## Notes on the Unpin requirement

Because of the signature of `Iterator::next`, it is always safe to move iterators between calls to
`next`. This makes unboxed, self-referential iterators unsound. We did not have `Pin` when we
designed the Iterator API.

However, in general, users can push unowned data outside of the iterator in a way they can't with
futures. Futures, usually, ultimately have to be `'static`, so they can spawned, but iterators
usually are consumed in a way that does not require them to own all of their data.

Therefore, it is potentially the case that generators restricted to not contain self-references are
sufficient for this use case. Propane intends to explore that possibility.

(Note: there is currently a bug where references are not allowed as arguments to generators at all.
This is because we need to expand lifetimes ourselves to get the correct lifetime ellision for
generators; an issue is open to track this problem.
