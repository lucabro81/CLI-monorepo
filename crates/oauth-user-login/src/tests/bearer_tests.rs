#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;

use super::BearerToken;

/// A fake API: answers each call with the next status and records the token it got.
fn api(statuses: &[u16]) -> (impl Fn(&str) -> u16, Rc<RefCell<Vec<String>>>) {
    let seen = Rc::new(RefCell::new(vec![]));
    let log = Rc::clone(&seen);
    let statuses = RefCell::new(statuses.to_vec());
    let send = move |token: &str| {
        log.borrow_mut().push(token.to_string());
        statuses.borrow_mut().remove(0)
    };
    (send, seen)
}

/// A renewer that records the rejected tokens it gets and answers `result` once.
fn renewer(result: Result<&str, &str>) -> (super::Renewer<String>, Rc<RefCell<Vec<String>>>) {
    let rejected = Rc::new(RefCell::new(vec![]));
    let log = Rc::clone(&rejected);
    let result = RefCell::new(Some(result.map(str::to_string).map_err(str::to_string)));
    let renew = Box::new(move |token: &str| {
        log.borrow_mut().push(token.to_string());
        result.borrow_mut().take().expect("renewed twice")
    });
    (renew, rejected)
}

#[allow(clippy::trivially_copy_pass_by_ref)] // BearerToken::send passes &T
fn unauthorized(status: &u16) -> bool {
    *status == 401
}

#[test]
fn a_success_is_returned_without_renewing() {
    let (send, seen) = api(&[200]);
    let (renew, rejected) = renewer(Ok("unused"));
    let token = BearerToken::new("at".to_string()).with_renewer(renew);

    assert_eq!(token.send(send, unauthorized), Ok(200));
    assert_eq!(*seen.borrow(), ["at"]);
    assert!(rejected.borrow().is_empty());
}

#[test]
fn a_401_renews_once_and_repeats_the_call_with_the_new_token() {
    // Issue #240: a token revoked before it expires; the call is repeated
    // with the renewed token instead of reporting the 401.
    let (send, seen) = api(&[401, 200]);
    let (renew, rejected) = renewer(Ok("fresh"));
    let token = BearerToken::new("at".to_string()).with_renewer(renew);

    assert_eq!(token.send(send, unauthorized), Ok(200));
    assert_eq!(*seen.borrow(), ["at", "fresh"]);
    assert_eq!(*rejected.borrow(), ["at"]);
    assert_eq!(token.current(), "fresh");
}

#[test]
fn a_second_401_is_returned_as_the_response_without_renewing_again() {
    let (send, seen) = api(&[401, 401]);
    let (renew, rejected) = renewer(Ok("fresh"));
    let token = BearerToken::new("at".to_string()).with_renewer(renew);

    assert_eq!(token.send(send, unauthorized), Ok(401));
    assert_eq!(seen.borrow().len(), 2);
    assert_eq!(rejected.borrow().len(), 1);
}

#[test]
fn a_failed_renewal_is_returned_and_the_call_is_not_repeated() {
    let (send, seen) = api(&[401]);
    let (renew, _) = renewer(Err("login expired"));
    let token = BearerToken::new("at".to_string()).with_renewer(renew);

    assert_eq!(token.send(send, unauthorized), Err("login expired".to_string()));
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(token.current(), "at");
}

#[test]
fn the_renewed_token_is_used_by_later_calls() {
    let (renew, rejected) = renewer(Ok("fresh"));
    let token = BearerToken::new("at".to_string()).with_renewer(renew);
    let (first, _) = api(&[401, 200]);
    token.send(first, unauthorized).unwrap();

    let (second, seen) = api(&[200]);
    token.send(second, unauthorized).unwrap();

    assert_eq!(*seen.borrow(), ["fresh"]);
    assert_eq!(rejected.borrow().len(), 1);
}

#[test]
fn without_a_renewer_a_401_is_returned_as_the_response() {
    let (send, seen) = api(&[401]);
    let token: BearerToken<String> = BearerToken::new("at".to_string());

    assert_eq!(token.send(send, unauthorized), Ok(401));
    assert_eq!(seen.borrow().len(), 1);
}
