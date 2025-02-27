// Copyright (c) 2021  Brendan Molloy <brendan@bbqsrc.net>,
//                     Ilya Solovyiov <ilya.solovyiov@gmail.com>,
//                     Kai Ren <tyranron@gmail.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Support for [custom][1] [`Parameter`]s.
//!
//! [1]: https://github.com/cucumber/cucumber-expressions#custom-parameter-types

use std::{collections::HashMap, fmt::Display, iter, vec};

use either::Either;
use nom::{AsChar, InputIter};

use crate::{Parameter, SingleExpression};

use super::{
    Expression, IntoRegexCharIter, ParameterIter, SingleExpressionIter,
    UnknownParameterError,
};

/// Parser of a [Cucumber Expressions][0] [AST] `Element` with [custom][1]
/// `Parameters` in mind.
///
/// Every [`Parameter`] should be represented by a single [`Regex`] capturing
/// group.
///
/// [`Regex`]: regex::Regex
/// [0]: https://github.com/cucumber/cucumber-expressions#readme
/// [1]: https://github.com/cucumber/cucumber-expressions#custom-parameter-types
/// [AST]: https://en.wikipedia.org/wiki/Abstract_syntax_tree
#[derive(Clone, Copy, Debug)]
pub struct WithCustom<Element, Parameters> {
    /// Parsed element of a [Cucumber Expressions][0] [AST].
    ///
    /// [0]: https://github.com/cucumber/cucumber-expressions#readme
    /// [AST]: https://en.wikipedia.org/wiki/Abstract_syntax_tree
    pub element: Element,

    /// Custom `Parameters` (in addition to [default ones][1]) to be used for
    /// expanding the `Element` into a [`Regex`].
    ///
    /// [`Regex`]: regex::Regex
    /// [1]: https://github.com/cucumber/cucumber-expressions#parameter-types
    pub parameters: Parameters,
}

/// Provider of custom [`Parameter`]s.
pub trait Provider<Input> {
    /// `<`[`Value`]` as `[`InputIter`]`>::`[`Item`].
    ///
    /// [`Item`]: InputIter::Item
    /// [`Value`]: Self::Value
    type Item: AsChar;

    /// Value matcher to be used in a [`Regex`].
    ///
    /// Should be represented by a single [`Regex`] capturing group.
    ///
    /// [`Regex`]: regex::Regex
    type Value: InputIter<Item = Self::Item>;

    /// Returns a [`Value`] matcher corresponding to the given `input`, if any.
    ///
    /// [`Value`]: Self::Value
    fn get(&self, input: &Input) -> Option<Self::Value>;
}

impl<'p, Input, Key, Value, S> Provider<Input> for &'p HashMap<Key, Value, S>
where
    Input: InputIter,
    <Input as InputIter>::Item: AsChar,
    Key: AsRef<str>,
    Value: AsRef<str>,
{
    type Item = char;
    type Value = &'p str;

    fn get(&self, input: &Input) -> Option<Self::Value> {
        self.iter().find_map(|(k, v)| {
            k.as_ref()
                .chars()
                .eq(input.iter_elements().map(AsChar::as_char))
                .then(|| v.as_ref())
        })
    }
}

impl<Input, Pars> IntoRegexCharIter<Input>
    for WithCustom<Expression<Input>, Pars>
where
    Input: Clone + Display + InputIter,
    <Input as InputIter>::Item: AsChar,
    Pars: Clone + Provider<Input>,
    <Pars as Provider<Input>>::Value: InputIter,
{
    type Iter = ExpressionWithParsIter<Input, Pars>;

    fn into_regex_char_iter(self) -> Self::Iter {
        let add_pars: fn(_) -> _ = |(item, parameters)| WithCustom {
            element: item,
            parameters,
        };
        let into_regex_char_iter: fn(_) -> _ =
            IntoRegexCharIter::into_regex_char_iter;
        iter::once(Ok('^'))
            .chain(
                self.element
                    .0
                    .into_iter()
                    .zip(iter::repeat(self.parameters))
                    .map(add_pars)
                    .flat_map(into_regex_char_iter),
            )
            .chain(iter::once(Ok('$')))
    }
}

// TODO: Replace with TAIT, once stabilized:
//       https://github.com/rust-lang/rust/issues/63063
/// [`IntoRegexCharIter::Iter`] for [`WithCustom`]`<`[`Expression`]`>`.
type ExpressionWithParsIter<I, P> = iter::Chain<
    iter::Chain<
        iter::Once<Result<char, UnknownParameterError<I>>>,
        iter::FlatMap<
            iter::Map<
                iter::Zip<vec::IntoIter<SingleExpression<I>>, iter::Repeat<P>>,
                fn(
                    (SingleExpression<I>, P),
                ) -> WithCustom<SingleExpression<I>, P>,
            >,
            SingleExprWithParsIter<I, P>,
            fn(
                WithCustom<SingleExpression<I>, P>,
            ) -> SingleExprWithParsIter<I, P>,
        >,
    >,
    iter::Once<Result<char, UnknownParameterError<I>>>,
>;

impl<Input, Pars> IntoRegexCharIter<Input>
    for WithCustom<SingleExpression<Input>, Pars>
where
    Input: Clone + Display + InputIter,
    <Input as InputIter>::Item: AsChar,
    Pars: Provider<Input>,
    <Pars as Provider<Input>>::Value: InputIter,
{
    type Iter = SingleExprWithParsIter<Input, Pars>;

    fn into_regex_char_iter(self) -> Self::Iter {
        use Either::{Left, Right};

        if let SingleExpression::Parameter(item) = self.element {
            Left(
                WithCustom {
                    element: item,
                    parameters: self.parameters,
                }
                .into_regex_char_iter(),
            )
        } else {
            Right(self.element.into_regex_char_iter())
        }
    }
}

// TODO: Replace with TAIT, once stabilized:
//       https://github.com/rust-lang/rust/issues/63063
/// [`IntoRegexCharIter::Iter`] for
/// [`WithCustom`]`<`[`SingleExpression`]`>`.
type SingleExprWithParsIter<I, P> = Either<
    <WithCustom<Parameter<I>, P> as IntoRegexCharIter<I>>::Iter,
    SingleExpressionIter<I>,
>;

impl<Input, P> IntoRegexCharIter<Input> for WithCustom<Parameter<Input>, P>
where
    Input: Clone + Display + InputIter,
    <Input as InputIter>::Item: AsChar,
    P: Provider<Input>,
    <P as Provider<Input>>::Value: InputIter,
{
    type Iter = WithParsIter<Input, P>;

    fn into_regex_char_iter(self) -> Self::Iter {
        use Either::{Left, Right};

        let ok: fn(_) -> _ = |c: <P::Value as InputIter>::Item| Ok(c.as_char());
        self.parameters.get(&self.element).map_or_else(
            || Right(self.element.into_regex_char_iter()),
            |v| {
                Left(
                    iter::once(Ok('('))
                        .chain(v.iter_elements().map(ok))
                        .chain(iter::once(Ok(')'))),
                )
            },
        )
    }
}

// TODO: Replace with TAIT, once stabilized:
//       https://github.com/rust-lang/rust/issues/63063
/// [`IntoRegexCharIter::Iter`] for [`WithCustom`]`<`[`Parameter`]`>`.
type WithParsIter<I, P> = Either<
    iter::Chain<
        iter::Chain<
            iter::Once<Result<char, UnknownParameterError<I>>>,
            iter::Map<
                <<P as Provider<I>>::Value as InputIter>::IterElem,
                fn(
                    <<P as Provider<I>>::Value as InputIter>::Item,
                ) -> Result<char, UnknownParameterError<I>>,
            >,
        >,
        iter::Once<Result<char, UnknownParameterError<I>>>,
    >,
    ParameterIter<I>,
>;

#[cfg(test)]
mod spec {
    use crate::expand::Error;

    use super::{Expression, HashMap, UnknownParameterError};

    #[test]
    fn custom_parameter() {
        let pars = HashMap::from([("custom", "custom")]);
        let expr = Expression::regex_with_parameters("{custom}", &pars)
            .unwrap_or_else(|e| panic!("failed: {}", e));

        assert_eq!(expr.as_str(), "^(custom)$");
    }

    #[test]
    fn default_parameter() {
        let pars = HashMap::from([("custom", "custom")]);
        let expr = Expression::regex_with_parameters("{}", &pars)
            .unwrap_or_else(|e| panic!("failed: {}", e));

        assert_eq!(expr.as_str(), "^(.*)$");
    }

    #[test]
    fn unknown_parameter() {
        let pars = HashMap::<String, String>::new();

        match Expression::regex_with_parameters("{custom}", &pars).unwrap_err()
        {
            Error::Expansion(UnknownParameterError { not_found }) => {
                assert_eq!(*not_found, "custom");
            }
            e @ (Error::Regex(_) | Error::Parsing(_)) => {
                panic!("wrong err: {}", e)
            }
        }
    }
}
