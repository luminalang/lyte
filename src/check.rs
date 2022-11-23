use crate::{query, Constraint, RefID, TEnv, TraitIndex, Type, TypeData, TypeKind, Types};

pub struct TypeContext<'a, D: TypeData> {
    pub(crate) tenv: &'a mut TEnv<D>,
    pub(crate) traits: &'a TraitIndex<D>,
    ehandler: ErrorHandler,
}

#[derive(Clone)]
pub enum ErrorHandler {
    PanicOnError,
    Cheap,
    Expensive,
}

#[derive(Debug, Clone)]
pub enum Error<D: TypeData> {
    Missmatch {
        left: Type<D>,
        right: Type<D>,
    },
    ParamAmountMissmatch {
        left: Type<D>,
        right: Type<D>,
    },

    ConstraintNotMet(Type<D>, Constraint<D>, Vec<query::Contender>),

    /// When the `cheap_error` flag is set
    Disgarded,
}

type CheckResult<D> = Result<(), Error<D>>;

impl<'a, D: TypeData> TypeContext<'a, D> {
    pub fn new(tenv: &'a mut TEnv<D>, traits: &'a TraitIndex<D>, ehandler: ErrorHandler) -> Self {
        Self { tenv, traits, ehandler }
    }

    pub fn check(&mut self, left: &Type<D>, right: &Type<D>) -> CheckResult<D> {
        match (&left.constr, &right.constr) {
            (TypeKind::Ref(lrid), TypeKind::Ref(rrid)) => {
                if *lrid == *rrid {
                    self.params(left, right)
                } else {
                    match (
                        self.tenv.get_type(*lrid).cloned(),
                        self.tenv.get_type(*rrid).cloned(),
                    ) {
                        (Some(l), Some(r)) => self.check(&l, &r),
                        (Some(l), None) => self.check(&l, right),
                        (None, Some(r)) => self.check(left, &r),
                        (None, None) => self.assign_refs_bidir(*lrid, left, *rrid, right),
                    }
                }
            }
            (TypeKind::Object(ltrid), TypeKind::Object(rtrid)) if ltrid == rtrid => {
                self.params(left, right)
            }
            (TypeKind::Concrete(lspec), TypeKind::Concrete(rspec)) if lspec == rspec => {
                self.params(left, right)
            }
            (TypeKind::Generic(lgid), TypeKind::Generic(rgid)) if lgid == rgid => {
                self.params(left, right)
            }

            (TypeKind::Ref(lrid), _) => match self.tenv.get_type(*lrid).cloned() {
                Some(assigned) => self.check(&assigned, right),
                None => self.assign_to_ref(*lrid, &left.params, right.clone()),
            },
            (_, TypeKind::Ref(rrid)) => match self.tenv.get_type(*rrid).cloned() {
                Some(assigned) => self.check(left, &assigned),
                None => self.assign_to_ref(*rrid, &right.params, left.clone()),
            },

            (TypeKind::Self_, TypeKind::Self_) => todo!(),
            (TypeKind::Self_, _) => todo!(),
            (_, TypeKind::Self_) => todo!(),

            _ => Err(self.ehandler.missmatch(self.tenv, left, right)),
        }
    }

    pub fn check_types(&mut self, left: &Types<D>, right: &Types<D>) -> Result<(), Error<D>> {
        left.iter()
            .zip(right)
            .try_for_each(|(l, r)| self.check(l, r))
    }

    fn assign_to_ref(
        &mut self,
        rid: RefID,
        rid_params: &Types<D>,
        given: Type<D>,
    ) -> CheckResult<D> {
        self.check_constraints_then_assign(rid, rid_params, given)
    }

    fn assign_refs_bidir(
        &mut self,
        lrid: RefID,
        left: &Type<D>,
        rrid: RefID,
        right: &Type<D>,
    ) -> CheckResult<D> {
        let apply_assignments = |self_: &mut Self| {
            let merged = self_
                .tenv
                .constraints(lrid)
                .iter()
                .chain(self_.tenv.constraints(rrid))
                .cloned()
                .collect();

            let mrid = self_.tenv.spawn_with_cons(merged);
            let type_ = Type::reference(left.meta.clone(), mrid, vec![]);
            self_.tenv.assign(lrid, type_.clone());
            self_.tenv.assign(rrid, type_);
        };

        match (left.params.len(), right.params.len()) {
            (x, y) if x == y => {
                apply_assignments(self);
                self.params(left, right)
            }
            // Do we need to do another RefID?
            //
            // Or perhaps we do want to assign one to the other?
            //
            // Or do we want to fail with inconsistent HKT?
            // Perhaps we want to fail with inconsistent if the other had constraints?
            (_, 0) => todo!("what to do with left.params"),
            (0, _) => todo!("what to do with right.params"),

            _ => Err(self.ehandler.param_amount_missmatch(self.tenv, left, right)),
        }
    }

    fn params(&mut self, left: &Type<D>, right: &Type<D>) -> CheckResult<D> {
        if left.params.len() != right.params.len() {
            Err(self.ehandler.param_amount_missmatch(self.tenv, left, right))
        } else {
            self.check_types(&left.params, &right.params)
        }
    }

    pub(crate) fn check_constraints_then_assign(
        &mut self,
        rid: RefID,
        rid_params: &Types<D>,
        // constrs: &[Constraint<D>],
        mut given: Type<D>,
    ) -> CheckResult<D> {
        if !rid_params.is_empty() {
            // NOTE: this means that we *need* to have consistent HKT usage.
            //
            // if we don't, the errors will be confusing. So; we might want to edge-case that.
            let at = given.params.len() - rid_params.len();
            let corresponding_of_higher_kinded = given.params.split_off(at);
            self.check_types(&corresponding_of_higher_kinded, rid_params)?;
        }

        let constrs = self.tenv.constraints(rid).to_vec();

        for con in constrs {
            let compatible = self
                .traits
                .select(self.tenv, con.trid.clone(), &con.params, &given);

            match compatible {
                Err(contendors) => {
                    return Err(Error::ConstraintNotMet(
                        given.clone(),
                        con.clone(),
                        contendors,
                    ))
                }
                Ok(mut matches) => {
                    let query::QuerySuccess { impl_, tenv, .. } = matches.remove(0);

                    if !matches.is_empty() {
                        // let's try actually creating this, otherwise I have no idea how to
                        // phrase/handle it.
                        //
                        // But ye, we definitely *need* this because otherwise we can't just
                        // randomly select the first type environment if it's not decisive.
                        todo!("ET: conflicting implementations/inference error??");
                    }

                    *self.tenv = tenv;

                    if !impl_.associated.is_empty() {
                        unimplemented!(
                            "we need to port this association instantiation to the new api"
                        );
                    };
                }
            }
        }

        self.tenv.assign(rid, given);

        Ok(())
    }
}

impl ErrorHandler {
    pub fn missmatch<D: TypeData>(
        &mut self,
        tenv: &TEnv<D>,
        left: &Type<D>,
        right: &Type<D>,
    ) -> Error<D> {
        match self {
            ErrorHandler::Cheap => Error::Disgarded,
            ErrorHandler::PanicOnError => panic!(
                "type missmatch: {} ⊑ {}",
                tenv.concretify_type(left),
                tenv.concretify_type(right)
            ),
            ErrorHandler::Expensive => Error::Missmatch {
                left: left.clone(),
                right: right.clone(),
            },
        }
    }

    pub fn param_amount_missmatch<D: TypeData>(
        &mut self,
        tenv: &TEnv<D>,
        left: &Type<D>,
        right: &Type<D>,
    ) -> Error<D> {
        match self {
            ErrorHandler::Cheap => Error::Disgarded,
            ErrorHandler::PanicOnError => panic!(
                "type missmatch: {} ⊑ {}",
                tenv.concretify_type(left),
                tenv.concretify_type(right)
            ),
            ErrorHandler::Expensive => Error::ParamAmountMissmatch {
                left: left.clone(),
                right: right.clone(),
            },
        }
    }
}
