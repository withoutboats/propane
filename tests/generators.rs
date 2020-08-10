#![feature(generators, generator_trait, try_trait)]

#[propane::generator]
fn foo() -> i32 {
    for n in 0..10 {
        yield n;
    }
}

#[test]
fn test_foo() {
    let mut foo = foo();
    for n in 0..10 {
        assert_eq!(foo.next(), Some(n));
    }
    assert!(foo.next().is_none());
}

#[propane::generator]
fn result() -> Result<i32, ()> {
    fn bar() -> Result<(), ()> {
        Err(())
    }

    for n in 0..5 {
        yield Ok(n);
    }

    bar()?;

    yield Ok(10); // will not be evaluated
}

#[test]
fn test_result() {
    let mut result = result();
    for n in 0..5 {
        assert_eq!(result.next(), Some(Ok(n)));
    }

    assert_eq!(result.next(), Some(Err(())));
    assert!(result.next().is_none())
}
