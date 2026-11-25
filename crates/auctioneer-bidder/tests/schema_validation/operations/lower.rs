//! Finite serde-emitter templates. Unknown syntax/calls are never free edges.

use super::model::{
    Action, Arm, Atom, Binding, Comparison, Exit, Function, IterationBound, Junction, Kind, Op,
    Origin, Pattern, Recipe, Site, StopRule, Ty, needs_cleanup,
};
use super::resolve::{Index, join, path};
use super::{
    ENTROPY_ENTRY, ExternalService, Primitive, PrimitiveSource, Refusal, RetainedScope,
    ServiceBinding, ServiceFact, TemporaryScope,
};
use quote::ToTokens as _;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{ParseStream, Parser as _};
use syn::{Expr, Pat, Stmt};

fn child(site: &[usize], ordinal: usize) -> Vec<usize> {
    let mut result = site.to_vec();
    result.push(ordinal);
    result
}

pub(super) fn recognize(
    index: &Index<'_>,
    primitives: &[PrimitiveSource],
    services: &[ServiceBinding],
) -> Result<Recipe, Refusal> {
    check_primitives(primitives)?;
    let mut recipe = Recipe {
        roots: index.roots().to_vec(),
        functions: BTreeMap::new(),
        initialization: index.initialization().clone(),
    };
    let mut pending = recipe.roots.clone();
    while let Some(id) = pending.pop() {
        if recipe.functions.contains_key(&id) {
            continue;
        }
        let source = index.source(id).ok_or(Refusal::UnclassifiedCall)?;
        let parameters = index.parameters(id)?;
        let mut lower = Lower {
            index,
            services,
            function: id,
            module: &source.module,
            scopes: vec![BTreeMap::new()],
            labels: Vec::new(),
            loop_depth: 0,
            returns: vec![Site {
                function: id,
                lexical: vec![0],
            }],
        };
        for parameter in &parameters {
            lower.bind(parameter.place.clone(), parameter.clone())?;
        }
        let mut body = lower.block(source.body, &[0]).map_err(|cause| {
            if (source.module.is_empty() && source.receiver.is_none())
                || matches!(&cause, Refusal::AtSource { .. })
            {
                cause
            } else {
                Refusal::AtSource {
                    function: source.identity.chars().take(128).collect(),
                    lexical_site: vec![0],
                    cause: Box::new(cause),
                }
            }
        })?;
        if let Kind::Scope { locals, .. } = &mut body.kind {
            locals.extend(
                parameters
                    .iter()
                    .filter(|parameter| needs_cleanup(parameter))
                    .cloned(),
            );
        }
        super::model::walk(&body, &mut |op| {
            if let Kind::Call { target, .. } = &op.kind {
                pending.push(*target);
            }
            Ok(())
        })?;
        recipe.functions.insert(
            id,
            Function {
                identity: source.identity.clone(),
                parameters,
                result: index.result(id)?,
                body,
            },
        );
    }
    Ok(recipe)
}

struct Lower<'a, 'source> {
    index: &'a Index<'source>,
    services: &'a [ServiceBinding],
    function: usize,
    module: &'a str,
    scopes: Vec<BTreeMap<String, Binding>>,
    labels: Vec<String>,
    loop_depth: usize,
    returns: Vec<Site>,
}

impl Lower<'_, '_> {
    fn site(&self, lexical: &[usize]) -> Site {
        Site {
            function: self.function,
            lexical: lexical.to_vec(),
        }
    }

    fn op(&self, lexical: &[usize], ty: Ty, kind: Kind) -> Op {
        let site = self.site(lexical);
        Op {
            value: Binding {
                declaration: None,
                ty,
                place: String::new(),
                borrowed: false,
                origin: Origin::Temporary(site.clone()),
            },
            site,
            kind,
        }
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn bind(&mut self, name: String, value: Binding) -> Result<(), Refusal> {
        self.scopes
            .last_mut()
            .ok_or(Refusal::UnsupportedSource)?
            .insert(name, value);
        Ok(())
    }

    fn primitive(
        &self,
        site: &[usize],
        ty: Ty,
        lemma: Primitive,
        callee: String,
        receiver: Option<Binding>,
        arguments: Vec<Op>,
    ) -> Op {
        self.op(
            site,
            ty,
            Kind::Primitive {
                lemma,
                callee,
                receiver,
                arguments,
                source_calls: Vec::new(),
            },
        )
    }

    fn block(&mut self, block: &syn::Block, site: &[usize]) -> Result<Op, Refusal> {
        self.scopes.push(BTreeMap::new());
        let result = (|| {
            let mut steps = Vec::new();
            let mut locals = Vec::new();
            let mut result = Ty::Unit;
            for (ordinal, statement) in block.stmts.iter().enumerate() {
                let here = child(site, ordinal);
                match statement {
                    Stmt::Local(local) => {
                        let init = local.init.as_ref().ok_or(Refusal::UnsupportedSource)?;
                        if init.diverge.is_some() {
                            return Err(Refusal::UnsupportedSource);
                        }
                        let value = self.expr(&init.expr, &child(&here, 0))?;
                        let (pattern, bindings) = self.pattern(&local.pat, &value.value, &here)?;
                        locals.extend(bindings.clone());
                        steps.push(self.op(
                            &here,
                            Ty::Unit,
                            Kind::Bind {
                                pattern,
                                bindings,
                                value: Box::new(value),
                            },
                        ));
                        result = Ty::Unit;
                    }
                    Stmt::Expr(value, semicolon) => {
                        let op = self.expr(value, &here)?;
                        result = if semicolon.is_some() {
                            Ty::Unit
                        } else {
                            op.value.ty.clone()
                        };
                        steps.push(op);
                    }
                    Stmt::Item(syn::Item::Const(item)) if item.ident == "_" => {
                        // Exact compiler recompile trigger: never a runtime read.
                        let Expr::Macro(mac) = item.expr.as_ref() else {
                            return Err(Refusal::UnsupportedSource);
                        };
                        if !mac.mac.path.is_ident("include_str")
                            || syn::parse2::<syn::LitStr>(mac.mac.tokens.clone()).is_err()
                        {
                            return Err(Refusal::UnsupportedSource);
                        }
                    }
                    Stmt::Item(_) | Stmt::Macro(_) => return Err(Refusal::UnsupportedSource),
                }
            }
            Ok(self.op(
                site,
                result,
                Kind::Scope {
                    label: None,
                    steps,
                    locals,
                },
            ))
        })();
        self.scopes.pop();
        result
    }

    fn expr(&mut self, expression: &Expr, site: &[usize]) -> Result<Op, Refusal> {
        self.expression(expression, site).map_err(|cause| {
            let source = self.index.source(self.function);
            if source
                .is_some_and(|function| function.module.is_empty() && function.receiver.is_none())
                || matches!(&cause, Refusal::AtSource { .. })
            {
                cause
            } else {
                Refusal::AtSource {
                    function: source.map_or(String::new(), |function| {
                        function.identity.chars().take(128).collect()
                    }),
                    lexical_site: site.to_vec(),
                    cause: Box::new(cause),
                }
            }
        })
    }

    fn expression(&mut self, expression: &Expr, site: &[usize]) -> Result<Op, Refusal> {
        // Keep operator temporaries out of this recursively re-entered frame.
        // Op has a wide owned Kind; unrelated match-arm locals otherwise make
        // debug-frame pressure grow with every emitted parenthesis/operator.
        match expression {
            Expr::Paren(expr) => self.expr(&expr.expr, &child(site, 0)),
            Expr::Group(expr) => self.expr(&expr.expr, &child(site, 0)),
            Expr::Block(expr) => self.block_expr(expr, site),
            Expr::Lit(expr) => self.literal_expr(expr, site),
            Expr::Path(expr) if expr.qself.is_none() => self.path_expr(expr, site),
            Expr::Reference(expr) => self.reference_expr(expr, site),
            Expr::Field(expr) => self.field_expr(expr, site),
            Expr::Call(expr) => self.call(expr, site),
            Expr::MethodCall(expr) => self.method(expr, site),
            Expr::Match(expr) => self.match_expr(expr, site),
            Expr::If(expr) => self.if_expr(expr, site),
            Expr::Binary(expr) => self.binary_expr(expr, site),
            Expr::Unary(expr) => self.unary_expr(expr, site),
            Expr::Assign(expr) => self.assign_expr(expr, site),
            Expr::Cast(expr) => self.cast_expr(expr, site),
            Expr::Tuple(expr) => self.tuple_expr(expr, site),
            Expr::Struct(expr) => self.construct(expr, site),
            Expr::Return(expr) => self.return_expr(expr, site),
            Expr::Break(expr) => self.break_expr(expr, site),
            Expr::Try(expr) => self.try_expr(expr, site),
            Expr::ForLoop(expr) => self.for_expr(expr, site),
            Expr::Macro(expr) if expr.mac.path.is_ident("matches") => self.matches_expr(expr, site),
            Expr::Loop(_) | Expr::While(_) => Err(Refusal::UnboundedTraversal),
            Expr::Array(_)
            | Expr::Async(_)
            | Expr::Await(_)
            | Expr::Closure(_)
            | Expr::Const(_)
            | Expr::Continue(_)
            | Expr::Index(_)
            | Expr::Infer(_)
            | Expr::Let(_)
            | Expr::Macro(_)
            | Expr::Path(_)
            | Expr::Range(_)
            | Expr::RawAddr(_)
            | Expr::Repeat(_)
            | Expr::TryBlock(_)
            | Expr::Unsafe(_)
            | Expr::Verbatim(_)
            | Expr::Yield(_)
            | _ => Err(Refusal::UnsupportedSource),
        }
    }

    fn block_expr(&mut self, expr: &syn::ExprBlock, site: &[usize]) -> Result<Op, Refusal> {
        let label = expr
            .label
            .as_ref()
            .map(|label| label.name.ident.to_string());
        if let Some(name) = &label {
            self.labels.push(name.clone());
        }
        let mut body = self.block(&expr.block, site)?;
        if label.is_some() {
            self.labels.pop();
        }
        if let Kind::Scope { label: target, .. } = &mut body.kind {
            *target = label;
        }
        Ok(body)
    }
    fn literal_expr(&self, expr: &syn::ExprLit, site: &[usize]) -> Result<Op, Refusal> {
        let (ty, atom) = match &expr.lit {
            syn::Lit::Bool(value) => (Ty::Bool, Atom::Bool(value.value)),
            syn::Lit::Int(_) | syn::Lit::Float(_) => (
                Ty::Scalar,
                Atom::Scalar(expr.lit.to_token_stream().to_string()),
            ),
            syn::Lit::Str(value) => (Ty::String, Atom::String(value.value())),
            syn::Lit::ByteStr(_)
            | syn::Lit::CStr(_)
            | syn::Lit::Byte(_)
            | syn::Lit::Char(_)
            | syn::Lit::Verbatim(_)
            | _ => return Err(Refusal::UnsupportedSource),
        };
        let mut op = self.op(site, ty, Kind::Value(atom));
        // String literals are borrowed static data, not owned Strings.
        op.value.borrowed = matches!(expr.lit, syn::Lit::Str(_));
        Ok(op)
    }
    fn path_expr(&self, expr: &syn::ExprPath, site: &[usize]) -> Result<Op, Refusal> {
        let name = path(&expr.path);
        if let Some(value) = self.lookup(&name) {
            return Ok(Op {
                site: self.site(site),
                value,
                kind: Kind::Value(Atom::Place),
            });
        }
        let qualified = join(self.module, &name);
        if let Some(ty) = self.index.static_type(&qualified) {
            return Ok(Op {
                site: self.site(site),
                value: Binding {
                    declaration: None,
                    ty: ty.clone(),
                    place: qualified.clone(),
                    borrowed: true,
                    origin: Origin::Static(qualified),
                },
                kind: Kind::Value(Atom::Place),
            });
        }
        if name == "None" {
            let result = self.index.result(self.function)?;
            return Ok(self.op(
                site,
                if matches!(result, Ty::Option(_)) {
                    result
                } else {
                    Ty::Option(Box::new(Ty::Unit))
                },
                Kind::Value(Atom::None),
            ));
        }
        Err(Refusal::UnclassifiedCall)
    }
    fn reference_expr(&mut self, expr: &syn::ExprReference, site: &[usize]) -> Result<Op, Refusal> {
        let value = self.expr(&expr.expr, &child(site, 0))?;
        let binding = value.value.clone();
        let mut op = self.op(
            site,
            binding.ty.clone(),
            Kind::Evaluate(
                Action::Borrow {
                    mutable: expr.mutability.is_some(),
                },
                vec![value],
            ),
        );
        op.value = binding;
        op.value.borrowed = true;
        Ok(op)
    }
    fn field_expr(&mut self, expr: &syn::ExprField, site: &[usize]) -> Result<Op, Refusal> {
        let base = self.expr(&expr.base, &child(site, 0))?;
        let syn::Member::Named(member) = &expr.member else {
            return Err(Refusal::ReceiverMismatch);
        };
        let Ty::Cache(owner) = &base.value.ty else {
            return Err(Refusal::ReceiverMismatch);
        };
        let ty = self
            .index
            .fields(owner)
            .and_then(|fields| fields.get(&member.to_string()))
            .ok_or(Refusal::ReceiverMismatch)?
            .clone();
        let binding = base.value.clone();
        let mut value = self.op(
            site,
            ty,
            Kind::Evaluate(Action::Field(member.to_string()), vec![base]),
        );
        value.value.borrowed = binding.borrowed;
        value.value.place = format!("{}.{}", binding.place, member);
        value.value.origin = Origin::Projection {
            base: Box::new(binding.origin),
            member: member.to_string(),
        };
        Ok(value)
    }
    fn match_expr(&mut self, expr: &syn::ExprMatch, site: &[usize]) -> Result<Op, Refusal> {
        let selected_expression = self.expr(&expr.expr, &child(site, 0))?;
        let selector_value = selected_expression.value.clone();
        let selector = if selector_value.ty == Ty::String {
            self.primitive(
                &child(site, 0),
                Ty::Bool,
                Primitive::StringAccess,
                "str::pattern_decision".to_owned(),
                Some(selector_value.clone()),
                vec![selected_expression],
            )
        } else {
            self.op(
                &child(site, 0),
                Ty::Bool,
                Kind::ScalarTest(Comparison::TypeDiscriminant, vec![selected_expression]),
            )
        };
        let mut arms = Vec::new();
        let mut ty = Ty::Unit;
        for (ordinal, arm) in expr.arms.iter().enumerate() {
            let here = child(site, ordinal.checked_add(1).ok_or(Refusal::Overflow)?);
            self.scopes.push(BTreeMap::new());
            let (source_pattern, source_guard) = if let Pat::Guard(guard) = &arm.pat {
                (guard.pat.as_ref(), Some(guard.guard.as_ref()))
            } else {
                (&arm.pat, None)
            };
            let (pattern, _) = self.pattern(source_pattern, &selector_value, &here)?;
            let guard = source_guard
                .map(|guard| self.expr(guard, &child(&here, 0)))
                .transpose()?;
            let body = self.expr(&arm.body, &child(&here, 1))?;
            ty = body.value.ty.clone();
            arms.push(Arm {
                pattern,
                guard,
                body,
            });
            self.scopes.pop();
        }
        Ok(self.op(
            site,
            ty,
            Kind::Choice {
                selector: Box::new(selector),
                arms,
            },
        ))
    }
    fn if_expr(&mut self, expr: &syn::ExprIf, site: &[usize]) -> Result<Op, Refusal> {
        let selector = self.expr(&expr.cond, &child(site, 0))?;
        if selector.value.ty != Ty::Bool {
            return Err(Refusal::ReceiverMismatch);
        }
        let yes = self.block(&expr.then_branch, &child(site, 1))?;
        let no = if let Some((_, no)) = &expr.else_branch {
            self.expr(no, &child(site, 2))?
        } else {
            self.op(&child(site, 2), Ty::Unit, Kind::Value(Atom::Unit))
        };
        let ty = if expr.else_branch.is_some() {
            no.value.ty.clone()
        } else {
            Ty::Unit
        };
        Ok(self.op(
            site,
            ty,
            Kind::Choice {
                selector: Box::new(selector),
                arms: vec![
                    Arm {
                        pattern: Pattern::Literal("true".to_owned()),
                        guard: None,
                        body: yes,
                    },
                    Arm {
                        pattern: Pattern::Literal("false".to_owned()),
                        guard: None,
                        body: no,
                    },
                ],
            },
        ))
    }
    fn binary_expr(&mut self, expr: &syn::ExprBinary, site: &[usize]) -> Result<Op, Refusal> {
        let left = self.expr(&expr.left, &child(site, 0))?;
        let right = self.expr(&expr.right, &child(site, 1))?;
        if matches!(expr.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            if left.value.ty != Ty::Bool || right.value.ty != Ty::Bool {
                return Err(Refusal::ReceiverMismatch);
            }
            return Ok(self.op(
                site,
                Ty::Bool,
                Kind::ShortCircuit(
                    if matches!(expr.op, syn::BinOp::And(_)) {
                        Junction::And
                    } else {
                        Junction::Or
                    },
                    vec![left, right],
                ),
            ));
        }
        if matches!(expr.op, syn::BinOp::BitAndAssign(_))
            && left.value.ty == Ty::Bool
            && right.value.ty == Ty::Bool
        {
            return Ok(self.op(
                site,
                Ty::Unit,
                Kind::Evaluate(Action::AndAssign(left.value.clone()), vec![left, right]),
            ));
        }
        if !matches!(
            expr.op,
            syn::BinOp::Eq(_)
                | syn::BinOp::Ne(_)
                | syn::BinOp::Lt(_)
                | syn::BinOp::Le(_)
                | syn::BinOp::Gt(_)
                | syn::BinOp::Ge(_)
        ) || left.value.ty != right.value.ty
        {
            return Err(Refusal::UnsupportedSource);
        }
        let comparison = comparison(&expr.op)?;
        if left.value.ty == Ty::String {
            return Ok(self.primitive(
                site,
                Ty::Bool,
                Primitive::StringAccess,
                format!("str::{comparison:?}"),
                Some(left.value.clone()),
                vec![left, right],
            ));
        }
        if !matches!(left.value.ty, Ty::Bool | Ty::Scalar) {
            return Err(Refusal::ReceiverMismatch);
        }
        Ok(self.op(
            site,
            Ty::Bool,
            Kind::ScalarTest(comparison, vec![left, right]),
        ))
    }
    fn unary_expr(&mut self, expr: &syn::ExprUnary, site: &[usize]) -> Result<Op, Refusal> {
        let value = self.expr(&expr.expr, &child(site, 0))?;
        let valid = match expr.op {
            syn::UnOp::Not(_) => value.value.ty == Ty::Bool,
            syn::UnOp::Neg(_) => value.value.ty == Ty::Scalar,
            syn::UnOp::Deref(_) => value.value.borrowed,
            _ => false,
        };
        if !valid {
            return Err(Refusal::ReceiverMismatch);
        }
        let action = match expr.op {
            syn::UnOp::Not(_) => Action::Not,
            syn::UnOp::Neg(_) => Action::Negate,
            syn::UnOp::Deref(_) => Action::Dereference,
            _ => return Err(Refusal::UnsupportedSource),
        };
        Ok(self.op(
            site,
            value.value.ty.clone(),
            Kind::Evaluate(action, vec![value]),
        ))
    }
    fn assign_expr(&mut self, expr: &syn::ExprAssign, site: &[usize]) -> Result<Op, Refusal> {
        let Expr::Path(target) = expr.left.as_ref() else {
            return Err(Refusal::UnsupportedSource);
        };
        let binding = self
            .lookup(&path(&target.path))
            .ok_or(Refusal::ReceiverMismatch)?;
        let value = self.expr(&expr.right, &child(site, 1))?;
        if binding.ty != Ty::Bool || value.value.ty != Ty::Bool {
            return Err(Refusal::ReceiverMismatch);
        }
        let left = self.expr(&expr.left, &child(site, 0))?;
        Ok(self.op(
            site,
            Ty::Unit,
            Kind::Evaluate(Action::Assign(binding), vec![left, value]),
        ))
    }
    fn cast_expr(&mut self, expr: &syn::ExprCast, site: &[usize]) -> Result<Op, Refusal> {
        let value = self.expr(&expr.expr, &child(site, 0))?;
        if value.value.ty != Ty::Scalar || self.index.ty(&expr.ty, self.module)? != Ty::Scalar {
            return Err(Refusal::ReceiverMismatch);
        }
        Ok(self.op(
            site,
            Ty::Scalar,
            Kind::Evaluate(Action::CastScalar, vec![value]),
        ))
    }
    fn tuple_expr(&mut self, expr: &syn::ExprTuple, site: &[usize]) -> Result<Op, Refusal> {
        let mut values = Vec::new();
        for (ordinal, value) in expr.elems.iter().enumerate() {
            values.push(self.expr(value, &child(site, ordinal))?);
        }
        Ok(self.op(
            site,
            Ty::Tuple(values.iter().map(|value| value.value.ty.clone()).collect()),
            Kind::Construct {
                fields: values
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, value)| (ordinal.to_string(), value))
                    .collect(),
                cleanup: Some(Primitive::Cleanup),
            },
        ))
    }
    fn return_expr(&mut self, expr: &syn::ExprReturn, site: &[usize]) -> Result<Op, Refusal> {
        let value = expr
            .expr
            .as_ref()
            .map(|value| self.expr(value, &child(site, 0)).map(Box::new))
            .transpose()?;
        Ok(self.op(
            site,
            Ty::Unit,
            Kind::Exit(
                Exit::Return(
                    self.returns
                        .last()
                        .ok_or(Refusal::UnsupportedSource)?
                        .clone(),
                ),
                value,
            ),
        ))
    }
    fn break_expr(&mut self, expr: &syn::ExprBreak, site: &[usize]) -> Result<Op, Refusal> {
        let label = expr.label.as_ref().map(|label| label.ident.to_string());
        if label
            .as_ref()
            .is_some_and(|name| !self.labels.contains(name))
            || (label.is_none() && self.loop_depth == 0)
        {
            return Err(Refusal::UnsupportedSource);
        }
        let value = expr
            .expr
            .as_ref()
            .map(|value| self.expr(value, &child(site, 0)).map(Box::new))
            .transpose()?;
        let ty = value
            .as_ref()
            .map_or(Ty::Unit, |result| result.value.ty.clone());
        Ok(self.op(
            site,
            ty,
            Kind::Exit(Exit::Break(label.unwrap_or_default()), value),
        ))
    }
    fn try_expr(&mut self, expr: &syn::ExprTry, site: &[usize]) -> Result<Op, Refusal> {
        let value = self.expr(&expr.expr, &child(site, 0))?;
        let binding = value.value.clone();
        let Ty::Option(inner) = &binding.ty else {
            return Err(Refusal::ReceiverMismatch);
        };
        let mut op = self.op(
            site,
            *inner.clone(),
            Kind::Exit(
                Exit::TryNone(
                    self.returns
                        .last()
                        .ok_or(Refusal::UnsupportedSource)?
                        .clone(),
                ),
                Some(Box::new(value)),
            ),
        );
        op.value.borrowed = binding.borrowed;
        op.value.origin = binding.origin;
        Ok(op)
    }
    fn for_expr(&mut self, expr: &syn::ExprForLoop, site: &[usize]) -> Result<Op, Refusal> {
        let receiver = self.expr(&expr.expr, &child(site, 0))?;
        let bound = iteration_bound(&receiver)?;
        let Ty::Iterator(element) = &receiver.value.ty else {
            return Err(Refusal::UnboundedTraversal);
        };
        let element_binding = Self::element(&receiver.value, *element.clone(), "member");
        self.scopes.push(BTreeMap::new());
        let (_, parameters) = self.pattern(&expr.pat, &element_binding, &child(site, 1))?;
        self.loop_depth = self.loop_depth.checked_add(1).ok_or(Refusal::Overflow)?;
        let body = self.block(&expr.body, &child(site, 1))?;
        self.loop_depth = self.loop_depth.checked_sub(1).ok_or(Refusal::Overflow)?;
        self.scopes.pop();
        Ok(self.op(
            site,
            Ty::Unit,
            Kind::Iterate {
                protocol: Primitive::IteratorProtocol,
                callee: "Iterator::next/for".to_owned(),
                stop: StopRule::BreakOrExhaustion,
                none_on_exhaustion: true,
                bound,
                receiver: receiver.value.clone(),
                parameters,
                body: Box::new(body),
                evaluation: Box::new(receiver),
            },
        ))
    }
    fn matches_expr(&mut self, expr: &syn::ExprMacro, site: &[usize]) -> Result<Op, Refusal> {
        let parser = |input: ParseStream<'_>| {
            let value: Expr = input.parse()?;
            input.parse::<syn::Token![,]>()?;
            let pattern = Pat::parse_multi_with_leading_vert(input)?;
            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
            Ok((value, pattern))
        };
        let (mut source_value, mut source_pattern) = parser
            .parse2(expr.mac.tokens.clone())
            .map_err(|_error| Refusal::UnsupportedSource)?;
        super::resolve::check_match_attributes(&mut source_value, &mut source_pattern)?;
        let value = self.expr(&source_value, &child(site, 0))?;
        self.scopes.push(BTreeMap::new());
        let (pattern, bindings) = self.pattern(&source_pattern, &value.value, &child(site, 1))?;
        self.scopes.pop();
        if !bindings.is_empty() {
            return Err(Refusal::UnsupportedSource);
        }
        let selector = if value.value.ty == Ty::String {
            self.primitive(
                &child(site, 0),
                Ty::Bool,
                Primitive::StringAccess,
                "str::pattern_decision".to_owned(),
                Some(value.value.clone()),
                vec![value],
            )
        } else {
            self.op(
                &child(site, 0),
                Ty::Bool,
                Kind::ScalarTest(Comparison::TypeDiscriminant, vec![value]),
            )
        };
        Ok(self.op(
            site,
            Ty::Bool,
            Kind::Choice {
                selector: Box::new(selector),
                arms: vec![
                    Arm {
                        pattern,
                        guard: None,
                        body: self.op(&child(site, 1), Ty::Bool, Kind::Value(Atom::Bool(true))),
                    },
                    Arm {
                        pattern: Pattern::Wild,
                        guard: None,
                        body: self.op(&child(site, 2), Ty::Bool, Kind::Value(Atom::Bool(false))),
                    },
                ],
            },
        ))
    }
    fn call(&mut self, call: &syn::ExprCall, site: &[usize]) -> Result<Op, Refusal> {
        let Expr::Path(callee) = call.func.as_ref() else {
            return Err(Refusal::UnknownCallback);
        };
        if callee.qself.is_some() {
            return Err(Refusal::UnclassifiedCall);
        }
        let raw = path(&callee.path);
        if let Some(binding) = self.lookup(&raw) {
            if let Ty::Callback(name) = &binding.ty {
                if name != "expired" || !call.args.is_empty() {
                    return Err(Refusal::UnknownCallback);
                }
                return Ok(self.op(
                    site,
                    Ty::Bool,
                    Kind::Service(self.service(ExternalService::Clock)?),
                ));
            }
            return Err(Refusal::UnknownCallback);
        }
        let name = self.index.canonical(self.module, &raw)?;
        let mut args = Vec::new();
        for (ordinal, argument) in call.args.iter().enumerate() {
            args.push(self.expr(argument, &child(site, ordinal))?);
        }
        if let Some(target) = self
            .index
            .function(&join(self.module, &name))
            .or_else(|| self.index.function(&name))
        {
            let parameters = self.index.parameters(target)?;
            if parameters.len() != args.len()
                || parameters
                    .iter()
                    .zip(&args)
                    .any(|(expected, actual)| expected.ty != actual.value.ty)
            {
                return Err(Refusal::ReceiverMismatch);
            }
            return Ok(self.op(
                site,
                self.index.result(target)?,
                Kind::Call {
                    target,
                    arguments: args.iter().map(|arg| arg.value.clone()).collect(),
                    evaluation: args,
                },
            ));
        }
        self.primitive_call(name, args, site)
    }

    fn primitive_call(&self, name: String, args: Vec<Op>, site: &[usize]) -> Result<Op, Refusal> {
        let types: Vec<_> = args.iter().map(|arg| arg.value.ty.clone()).collect();
        let (lemma, result) = match (name.as_str(), types.as_slice()) {
            ("Some", [ty]) => {
                return Ok(self.op(
                    site,
                    Ty::Option(Box::new(ty.clone())),
                    Kind::Construct {
                        fields: args
                            .into_iter()
                            .map(|arg| ("Some".to_owned(), arg))
                            .collect(),
                        cleanup: Some(Primitive::Cleanup),
                    },
                ));
            }
            ("crate::schema::staging::formats::uuid", [Ty::String]) => {
                (Primitive::UuidFormat, Ty::Bool)
            }
            ("crate::schema::staging::formats::hostname", [Ty::String]) => {
                (Primitive::HostnameFormat, Ty::Bool)
            }
            ("jsonschema::__private::types::is_integer_draft4", [Ty::Number]) => {
                (Primitive::Numeric, Ty::Bool)
            }
            (
                "jsonschema::__private::numeric::ge"
                | "jsonschema::__private::numeric::gt"
                | "jsonschema::__private::numeric::le"
                | "jsonschema::__private::numeric::lt"
                | "jsonschema::__private::numeric::eq",
                [Ty::Number, Ty::Scalar],
            ) => (Primitive::Numeric, Ty::Bool),
            ("jsonschema::__private::cmp::equal", [Ty::Value, Ty::Value]) => {
                (Primitive::Equality, Ty::Bool)
            }
            ("jsonschema::__private::unique_items::is_unique", [Ty::Array]) => {
                (Primitive::Uniqueness, Ty::Bool)
            }
            ("regex_automata::Input::new", [Ty::String]) => (Primitive::RegexSearch, Ty::Input),
            ("std::collections::HashMap::new", []) => {
                return self.state_map_constructor(site, name, args);
            }
            _ => return Err(Refusal::UnclassifiedCall),
        };
        Ok(self.primitive(site, result, lemma, name, None, args))
    }

    fn state_map_constructor(
        &self,
        site: &[usize],
        name: String,
        args: Vec<Op>,
    ) -> Result<Op, Refusal> {
        let service = self.op(
            site,
            Ty::Unit,
            Kind::Service(self.service(ExternalService::HostEntropy)?),
        );
        let mut seed_default = self.primitive(
            site,
            Ty::Named("std::hash::RandomState".to_owned()),
            Primitive::StateMapConstruction,
            "std::hash::RandomState::default".to_owned(),
            None,
            Vec::new(),
        );
        if let Kind::Primitive { source_calls, .. } = &mut seed_default.kind {
            source_calls.push(service);
        }
        let mut map_default = self.primitive(
            site,
            Ty::Named("StateMap".to_owned()),
            Primitive::StateMapConstruction,
            "std::collections::HashMap::default".to_owned(),
            None,
            Vec::new(),
        );
        if let Kind::Primitive { source_calls, .. } = &mut map_default.kind {
            source_calls.push(seed_default);
        }
        let mut op = self.primitive(
            site,
            Ty::Named("StateMap".to_owned()),
            Primitive::StateMapConstruction,
            name,
            None,
            args,
        );
        if let Kind::Primitive { source_calls, .. } = &mut op.kind {
            source_calls.push(map_default);
        }
        Ok(op)
    }

    fn method(&mut self, call: &syn::ExprMethodCall, site: &[usize]) -> Result<Op, Refusal> {
        if call.turbofish.is_some() {
            return Err(Refusal::UnsupportedSource);
        }
        let receiver = self.expr(&call.receiver, &child(site, 0))?;
        let name = call.method.to_string();
        if matches!(&receiver.value.ty, Ty::Named(owner) if owner == "aho_corasick::AhoCorasick") {
            return Err(Refusal::UnsupportedNestedLiteralOwner);
        }
        if let Ty::Cache(owner) = &receiver.value.ty
            && let Some(target) = self.index.function(&join(owner, &name))
        {
            let mut args = vec![receiver];
            for (ordinal, arg) in call.args.iter().enumerate() {
                args.push(self.expr(
                    arg,
                    &child(site, ordinal.checked_add(1).ok_or(Refusal::Overflow)?),
                )?);
            }
            let parameters = self.index.parameters(target)?;
            if parameters.len() != args.len()
                || parameters
                    .iter()
                    .zip(&args)
                    .any(|(expected, actual)| expected.ty != actual.value.ty)
            {
                return Err(Refusal::ReceiverMismatch);
            }
            return Ok(self.op(
                site,
                self.index.result(target)?,
                Kind::Call {
                    target,
                    arguments: args.iter().map(|arg| arg.value.clone()).collect(),
                    evaluation: args,
                },
            ));
        }
        if matches!(name.as_str(), "all" | "map" | "is_some_and") {
            return self.callback_method(call, site, receiver, &name);
        }
        self.primitive_method(call, site, receiver, &name)
    }

    fn callback_method(
        &mut self,
        call: &syn::ExprMethodCall,
        site: &[usize],
        receiver: Op,
        name: &str,
    ) -> Result<Op, Refusal> {
        if call.args.len() != 1 {
            return Err(Refusal::UnknownCallback);
        }
        let element = match (&receiver.value.ty, name) {
            (Ty::Iterator(element), "all") | (Ty::Option(element), "map" | "is_some_and") => {
                *element.clone()
            }
            _ => return Err(Refusal::ReceiverMismatch),
        };
        let argument = call.args.first().ok_or(Refusal::UnknownCallback)?;
        let Expr::Closure(closure) = argument else {
            return Err(Refusal::UnknownCallback);
        };
        if closure.inputs.len() != 1 || closure.capture.is_some() || closure.asyncness.is_some() {
            return Err(Refusal::UnknownCallback);
        }
        self.scopes.push(BTreeMap::new());
        let element_binding = Self::element(
            &receiver.value,
            element,
            if name == "all" { "member" } else { "Some" },
        );
        let (_, parameters) = self.pattern(
            closure.inputs.first().ok_or(Refusal::UnknownCallback)?,
            &element_binding,
            &child(site, 1),
        )?;
        self.returns.push(self.site(&child(site, 1)));
        let body = self.expr(&closure.body, &child(site, 1))?;
        self.returns.pop();
        self.scopes.pop();
        if name != "map" && body.value.ty != Ty::Bool {
            return Err(Refusal::ReceiverMismatch);
        }
        let ty = if name == "map" {
            Ty::Option(Box::new(body.value.ty.clone()))
        } else {
            Ty::Bool
        };
        Ok(self.op(
            site,
            ty,
            if name == "all" {
                Kind::Iterate {
                    protocol: Primitive::IteratorProtocol,
                    callee: "Iterator::all".to_owned(),
                    stop: StopRule::FirstFalseOrExhaustion,
                    none_on_exhaustion: true,
                    bound: iteration_bound(&receiver)?,
                    receiver: receiver.value.clone(),
                    parameters,
                    body: Box::new(body),
                    evaluation: Box::new(receiver),
                }
            } else {
                Kind::Callback {
                    protocol: Primitive::ValueAccess,
                    callee: format!("Option::{name}"),
                    receiver: receiver.value.clone(),
                    parameters,
                    body: Box::new(body),
                    evaluation: Box::new(receiver),
                }
            },
        ))
    }

    fn primitive_method(
        &mut self,
        call: &syn::ExprMethodCall,
        site: &[usize],
        receiver: Op,
        name: &str,
    ) -> Result<Op, Refusal> {
        let receiver_binding = receiver.value.clone();
        let mut args = vec![receiver];
        for (ordinal, arg) in call.args.iter().enumerate() {
            args.push(self.expr(
                arg,
                &child(site, ordinal.checked_add(1).ok_or(Refusal::Overflow)?),
            )?);
        }
        let others: Vec<_> = args
            .iter()
            .skip(1)
            .map(|arg| arg.value.ty.clone())
            .collect();
        let (lemma, result, borrowed) =
            Self::method_signature(&receiver_binding.ty, name, &others)?;
        // Input/search protocol selection is a semantic premise, not any bool.
        if name == "earliest"
            && !matches!(call.args.first(), Some(Expr::Lit(literal)) if matches!(&literal.lit, syn::Lit::Bool(value) if value.value))
        {
            return Err(Refusal::UnsupportedSource);
        }
        let mut op = self.primitive(
            site,
            result,
            lemma,
            format!("{:?}::{name}", receiver_binding.ty),
            Some(receiver_binding.clone()),
            args,
        );
        if name == "search_half_with" {
            let program =
                program_origin(&receiver_binding.origin).ok_or(Refusal::ReceiverMismatch)?;
            let Kind::Primitive { arguments, .. } = &op.kind else {
                return Err(Refusal::ReceiverMismatch);
            };
            let cache = arguments.get(1).ok_or(Refusal::ReceiverMismatch)?;
            if !matches!(&cache.value.origin, Origin::Projection { member, .. } if *member == cache_field(program)?)
            {
                return Err(Refusal::ReceiverMismatch);
            }
        }
        op.value.borrowed = borrowed;
        op.value.place = receiver_binding.place;
        op.value.origin = receiver_binding.origin;
        Ok(op)
    }

    fn method_signature(
        receiver_type: &Ty,
        name: &str,
        others: &[Ty],
    ) -> Result<(Primitive, Ty, bool), Refusal> {
        Ok(match (receiver_type, name, others) {
            (
                Ty::Value,
                "is_string" | "is_number" | "is_boolean" | "is_null" | "is_array" | "is_object",
                [],
            )
            | (Ty::String | Ty::Array | Ty::Map, "is_empty", [])
            | (Ty::Option(_), "is_some", []) => (Primitive::ValueAccess, Ty::Bool, false),
            (Ty::String, "as_str", []) => (Primitive::StringAccess, Ty::String, true),
            (Ty::String | Ty::Array | Ty::Map, "len", []) => {
                (Primitive::ValueAccess, Ty::Scalar, false)
            }
            (Ty::String, "starts_with", [Ty::String]) => (Primitive::StringAccess, Ty::Bool, false),
            (Ty::Map, "contains_key", [Ty::String]) => (Primitive::MapAccess, Ty::Bool, false),
            (Ty::Array, "iter", []) | (Ty::Map, "values", []) => (
                Primitive::IteratorProtocol,
                Ty::Iterator(Box::new(Ty::Value)),
                true,
            ),
            (Ty::Map, "iter", []) => (
                Primitive::IteratorProtocol,
                Ty::Iterator(Box::new(Ty::Tuple(vec![Ty::String, Ty::Value]))),
                true,
            ),
            (Ty::Map, "keys", []) => (
                Primitive::IteratorProtocol,
                Ty::Iterator(Box::new(Ty::String)),
                true,
            ),
            (Ty::LazyRegex(_), "as_ref", []) => {
                (Primitive::Readiness, Ty::Option(Box::new(Ty::Regex)), true)
            }
            (Ty::Option(inner), "as_ref", []) => {
                (Primitive::ValueAccess, Ty::Option(inner.clone()), true)
            }
            (Ty::Regex, "create_cache", []) => (
                Primitive::CacheConstruction,
                Ty::Cache("regex_automata::meta::Cache".to_owned()),
                false,
            ),
            (Ty::Regex, "search_half_with", [Ty::Cache(cache), Ty::Input])
                if cache == "regex_automata::meta::Cache" =>
            {
                (
                    Primitive::RegexSearch,
                    Ty::Option(Box::new(Ty::Scalar)),
                    false,
                )
            }
            (Ty::Input, "earliest", [Ty::Bool]) => (Primitive::RegexSearch, Ty::Input, false),
            _ => {
                return Err(if matches!(receiver_type, Ty::Named(_)) {
                    Refusal::ReceiverMismatch
                } else {
                    Refusal::UnclassifiedCall
                });
            }
        })
    }

    fn construct(&mut self, expr: &syn::ExprStruct, site: &[usize]) -> Result<Op, Refusal> {
        if expr.rest.is_some() {
            return Err(Refusal::UnsupportedSource);
        }
        let raw = path(&expr.path);
        let owner = if self.index.fields(&join(self.module, &raw)).is_some() {
            join(self.module, &raw)
        } else {
            raw
        };
        let expected = self.index.fields(&owner).ok_or(Refusal::ReceiverMismatch)?;
        if expected.len() != expr.fields.len() {
            return Err(Refusal::ReceiverMismatch);
        }
        let mut seen = BTreeSet::new();
        let mut fields = Vec::new();
        for (ordinal, field) in expr.fields.iter().enumerate() {
            let syn::Member::Named(member) = &field.member else {
                return Err(Refusal::ReceiverMismatch);
            };
            let name = member.to_string();
            if !seen.insert(name.clone()) {
                return Err(Refusal::ReceiverMismatch);
            }
            let value = self.expr(&field.expr, &child(site, ordinal))?;
            if expected.get(&name) != Some(&value.value.ty) {
                return Err(Refusal::ReceiverMismatch);
            }
            if value.value.ty == Ty::Cache("regex_automata::meta::Cache".to_owned()) {
                let program =
                    program_origin(&value.value.origin).ok_or(Refusal::ReceiverMismatch)?;
                if name != cache_field(program)? {
                    return Err(Refusal::ReceiverMismatch);
                }
            }
            fields.push((name, value));
        }
        Ok(self.op(
            site,
            Ty::Cache(owner),
            Kind::Construct {
                fields,
                cleanup: Some(Primitive::Cleanup),
            },
        ))
    }

    fn element(receiver: &Binding, ty: Ty, member: &str) -> Binding {
        Binding {
            declaration: None,
            ty,
            place: receiver.place.clone(),
            borrowed: receiver.borrowed,
            origin: Origin::Projection {
                base: Box::new(receiver.origin.clone()),
                member: member.to_owned(),
            },
        }
    }

    fn pattern(
        &mut self,
        pattern: &Pat,
        value: &Binding,
        site: &[usize],
    ) -> Result<(Pattern, Vec<Binding>), Refusal> {
        match pattern {
            Pat::Wild(_) => Ok((Pattern::Wild, Vec::new())),
            Pat::Ident(name) if name.subpat.is_none() => {
                let mut binding = value.clone();
                binding.declaration = Some(self.site(site));
                binding.place = name.ident.to_string();
                self.bind(binding.place.clone(), binding.clone())?;
                Ok((Pattern::Binding(name.ident.to_string()), vec![binding]))
            }
            Pat::Type(pat) if self.index.ty(&pat.ty, self.module)? == value.ty => {
                self.pattern(&pat.pat, value, site)
            }
            Pat::Reference(pat) => self.pattern(&pat.pat, value, site),
            Pat::Paren(pat) => self.pattern(&pat.pat, value, site),
            Pat::Tuple(pat) => {
                let Ty::Tuple(types) = &value.ty else {
                    return Err(Refusal::ReceiverMismatch);
                };
                if pat.elems.len() != types.len() {
                    return Err(Refusal::ReceiverMismatch);
                }
                let mut patterns = Vec::new();
                let mut bindings = Vec::new();
                for (ordinal, (member_pattern, ty)) in pat.elems.iter().zip(types).enumerate() {
                    let element = Self::element(value, ty.clone(), &ordinal.to_string());
                    let (member, bound) =
                        self.pattern(member_pattern, &element, &child(site, ordinal))?;
                    patterns.push(member);
                    bindings.extend(bound);
                }
                Ok((Pattern::Tuple(patterns), bindings))
            }
            Pat::TupleStruct(pat) => self.variant_pattern(pat, value, site),
            Pat::Path(pat)
                if value.ty == Ty::Value
                    && matches!(
                        self.index
                            .canonical(self.module, &path(&pat.path))?
                            .as_str(),
                        "serde_json::Value::Null"
                            | "jsonschema::__private::serde_json::Value::Null"
                    ) =>
            {
                Ok((
                    Pattern::Variant("Value::Null".to_owned(), Vec::new()),
                    Vec::new(),
                ))
            }
            Pat::Lit(pat) => {
                let compatible = match &pat.lit {
                    syn::Lit::Str(_) => value.ty == Ty::String,
                    syn::Lit::Bool(_) => value.ty == Ty::Bool,
                    syn::Lit::Int(_) => value.ty == Ty::Scalar,
                    syn::Lit::ByteStr(_)
                    | syn::Lit::CStr(_)
                    | syn::Lit::Byte(_)
                    | syn::Lit::Char(_)
                    | syn::Lit::Float(_)
                    | syn::Lit::Verbatim(_)
                    | _ => false,
                };
                if !compatible {
                    return Err(Refusal::ReceiverMismatch);
                }
                Ok((
                    Pattern::Literal(pat.lit.to_token_stream().to_string()),
                    Vec::new(),
                ))
            }
            Pat::Or(pat) => {
                let mut patterns = Vec::new();
                for (ordinal, case) in pat.cases.iter().enumerate() {
                    let (alternative, bindings) =
                        self.pattern(case, value, &child(site, ordinal))?;
                    if !bindings.is_empty() {
                        return Err(Refusal::UnsupportedSource);
                    }
                    patterns.push(alternative);
                }
                Ok((Pattern::Or(patterns), Vec::new()))
            }
            Pat::Const(_)
            | Pat::Guard(_)
            | Pat::Ident(_)
            | Pat::Macro(_)
            | Pat::Path(_)
            | Pat::Range(_)
            | Pat::Rest(_)
            | Pat::Slice(_)
            | Pat::Struct(_)
            | Pat::Type(_)
            | Pat::Verbatim(_)
            | _ => Err(Refusal::UnsupportedSource),
        }
    }

    fn variant_pattern(
        &mut self,
        pat: &syn::PatTupleStruct,
        value: &Binding,
        site: &[usize],
    ) -> Result<(Pattern, Vec<Binding>), Refusal> {
        let name = self.index.canonical(self.module, &path(&pat.path))?;
        let ty = match (&value.ty, name.as_str()) {
            (
                Ty::Value,
                "serde_json::Value::String" | "jsonschema::__private::serde_json::Value::String",
            ) => Ty::String,
            (
                Ty::Value,
                "serde_json::Value::Number" | "jsonschema::__private::serde_json::Value::Number",
            ) => Ty::Number,
            (
                Ty::Value,
                "serde_json::Value::Bool" | "jsonschema::__private::serde_json::Value::Bool",
            ) => Ty::Bool,
            (
                Ty::Value,
                "serde_json::Value::Array" | "jsonschema::__private::serde_json::Value::Array",
            ) => Ty::Array,
            (
                Ty::Value,
                "serde_json::Value::Object" | "jsonschema::__private::serde_json::Value::Object",
            ) => Ty::Map,
            (Ty::Option(inner), "Some") => *inner.clone(),
            _ => return Err(Refusal::ReceiverMismatch),
        };
        if pat.elems.len() != 1 {
            return Err(Refusal::ReceiverMismatch);
        }
        let mut element = Self::element(value, ty, &name);
        element.borrowed = true;
        let (inner, bindings) = self.pattern(
            pat.elems.first().ok_or(Refusal::ReceiverMismatch)?,
            &element,
            &child(site, 0),
        )?;
        Ok((Pattern::Variant(name, vec![inner]), bindings))
    }

    fn service(&self, service: ExternalService) -> Result<ServiceBinding, Refusal> {
        let mut matches = self
            .services
            .iter()
            .filter(|binding| binding.service == service);
        let binding = matches
            .next()
            .ok_or(Refusal::MissingServiceBinding(service))?;
        if matches.next().is_some() {
            return Err(Refusal::ServiceIdentity(service));
        }
        let (entry, receiver, retained, temporary) = match service {
            ExternalService::Clock => (
                "fixture::RequestClock::expired",
                "fixture::RequestClock",
                RetainedScope::Caller,
                TemporaryScope::OutsideCache,
            ),
            ExternalService::HostEntropy => (
                ENTROPY_ENTRY,
                "std::hash::RandomState",
                RetainedScope::ThreadAndProcess,
                TemporaryScope::CacheConstruction,
            ),
        };
        if binding.entry != entry || binding.receiver != receiver {
            return Err(Refusal::ServiceIdentity(service));
        }
        if binding.retained != retained || binding.temporary != temporary {
            return Err(Refusal::ServiceLifetime(service));
        }
        for fact in [
            ServiceFact::Work,
            ServiceFact::TemporaryStorage,
            ServiceFact::RetainedState,
            ServiceFact::Stack,
        ]
        .into_iter()
        .chain((service == ExternalService::HostEntropy).then_some(ServiceFact::ThreadMultiplicity))
        .chain(
            (service == ExternalService::HostEntropy).then_some(ServiceFact::ProcessRetainedState),
        ) {
            if !binding.required_facts.contains(&fact) {
                return Err(Refusal::MissingServiceFact(service, fact));
            }
        }
        Ok(binding.clone())
    }
}

fn program_origin(origin: &Origin) -> Option<&str> {
    match origin {
        Origin::Static(program) => Some(program),
        Origin::Projection { base, .. } => program_origin(base),
        Origin::Parameter { .. } | Origin::Temporary(_) => None,
    }
}

fn comparison(operator: &syn::BinOp) -> Result<Comparison, Refusal> {
    Ok(match operator {
        syn::BinOp::Eq(_) => Comparison::Eq,
        syn::BinOp::Ne(_) => Comparison::Ne,
        syn::BinOp::Lt(_) => Comparison::Lt,
        syn::BinOp::Le(_) => Comparison::Le,
        syn::BinOp::Gt(_) => Comparison::Gt,
        syn::BinOp::Ge(_) => Comparison::Ge,
        syn::BinOp::Add(_)
        | syn::BinOp::Sub(_)
        | syn::BinOp::Mul(_)
        | syn::BinOp::Div(_)
        | syn::BinOp::Rem(_)
        | syn::BinOp::And(_)
        | syn::BinOp::Or(_)
        | syn::BinOp::BitXor(_)
        | syn::BinOp::BitAnd(_)
        | syn::BinOp::BitOr(_)
        | syn::BinOp::Shl(_)
        | syn::BinOp::Shr(_)
        | syn::BinOp::AddAssign(_)
        | syn::BinOp::SubAssign(_)
        | syn::BinOp::MulAssign(_)
        | syn::BinOp::DivAssign(_)
        | syn::BinOp::RemAssign(_)
        | syn::BinOp::BitXorAssign(_)
        | syn::BinOp::BitAndAssign(_)
        | syn::BinOp::BitOrAssign(_)
        | syn::BinOp::ShlAssign(_)
        | syn::BinOp::ShrAssign(_)
        | _ => return Err(Refusal::UnsupportedSource),
    })
}

fn iteration_bound(receiver: &Op) -> Result<IterationBound, Refusal> {
    let Kind::Primitive {
        lemma: Primitive::IteratorProtocol,
        receiver: Some(collection),
        ..
    } = &receiver.kind
    else {
        return Err(Refusal::UnboundedTraversal);
    };
    let Ty::Iterator(element) = &receiver.value.ty else {
        return Err(Refusal::UnboundedTraversal);
    };
    if !collection.borrowed || !matches!(collection.ty, Ty::Array | Ty::Map) {
        return Err(Refusal::UnboundedTraversal);
    }
    Ok(IterationBound {
        collection: collection.clone(),
        element: *element.clone(),
    })
}

fn cache_field(program: &str) -> Result<String, Refusal> {
    let symbol = program
        .rsplit("::")
        .next()
        .ok_or(Refusal::ReceiverMismatch)?;
    let mut field = symbol
        .strip_suffix("_REGEX")
        .ok_or(Refusal::ReceiverMismatch)?
        .to_lowercase();
    field.push_str("_cache");
    Ok(field)
}

fn check_primitives(primitives: &[PrimitiveSource]) -> Result<(), Refusal> {
    let mut seen = BTreeSet::new();
    for source in primitives {
        if !seen.insert(source.primitive) {
            return Err(Refusal::PrimitiveSemanticsChanged);
        }
        // Exact syntactic semantic premises, not a digest whitelist. Changing
        // uppercase rejection, scan order, buffer size, parser, output handling
        // or constructor delegation fails even with a self-consistent new hash.
        let (callee, receiver, expected): (&str, &str, syn::ItemFn) = match source.primitive {
            Primitive::UuidFormat => (
                "crate::schema::staging::formats::uuid",
                "str",
                syn::parse_quote! {
                    fn uuid(value: &str) -> bool {
                        if value.bytes().any(|byte| byte.is_ascii_uppercase()) { return false; }
                        let mut decoded = [0_u8; 16];
                        uuid_simd::parse_hyphenated(value.as_bytes(), decoded.as_out()).is_ok()
                    }
                },
            ),
            Primitive::StateMapConstruction => (
                "std::collections::HashMap::new",
                "std::collections::HashMap",
                syn::parse_quote! {
                    fn new() -> HashMap<K, V, RandomState> { Default::default() }
                },
            ),
            Primitive::ValueAccess
            | Primitive::StringAccess
            | Primitive::MapAccess
            | Primitive::IteratorProtocol
            | Primitive::Numeric
            | Primitive::RegexSearch
            | Primitive::CacheConstruction
            | Primitive::Readiness
            | Primitive::HostnameFormat
            | Primitive::Equality
            | Primitive::Uniqueness
            | Primitive::Cleanup => return Err(Refusal::PrimitiveSemanticsChanged),
        };
        let actual = source.body.to_token_stream().to_string();
        if source.callee != callee
            || source.receiver != receiver
            || actual != expected.to_token_stream().to_string()
            || source.source_sha256 != hex::encode(Sha256::digest(actual.as_bytes()))
        {
            return Err(Refusal::PrimitiveSemanticsChanged);
        }
    }
    Ok(())
}
