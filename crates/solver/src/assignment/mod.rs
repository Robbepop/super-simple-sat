mod model;
mod trail;
mod watch_list;

pub use self::model::{
    LastModel,
    Model,
    ModelIter,
};
use self::{
    trail::{
        DecisionLevel,
        Trail,
    },
    watch_list::WatchList,
};
use bounded::{
    bounded_map,
    BoundedMap,
};
use crate::{
    clause_db::ClauseRef,
    decider::InformDecider,
    Bool,
    ClauseDb,
    Literal,
    Sign,
    Variable,
};

/// Errors that may be encountered when operating on the assignment.
#[derive(Debug, PartialEq, Eq)]
pub enum AssignmentError {
    /// When trying to create a model from an indeterminate assignment.
    UnexpectedIndeterminateAssignment,
    /// Variable invalid for the current assignment.
    InvalidVariable,
    /// When trying to assign a variable that has already been assigned.
    AlreadyAssigned,
    /// When trying to assign a conflict.
    Conflict,
}

impl AssignmentError {
    /// Returns `true` if the assignment error was caused by a conflict.
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict)
    }
}

/// Allows to enqueue new literals into the propagation queue.
#[derive(Debug)]
pub struct PropagationEnqueuer<'a> {
    queue: &'a mut Trail,
}

impl<'a> PropagationEnqueuer<'a> {
    /// Returns a new wrapper around the given propagation queue.
    fn new(queue: &'a mut Trail) -> Self {
        Self { queue }
    }

    /// Enqueues a new literal to the propagation queue.
    ///
    /// # Errors
    ///
    /// - If the literal has already been satisfied.
    /// - If the literal is in conflict with the current assignment. This will
    ///   also clear the propagation queue.
    pub fn push(
        &mut self,
        literal: Literal,
        assignment: &mut VariableAssignment,
    ) -> Result<(), AssignmentError> {
        self.queue.push(literal, assignment)
    }
}

/// The actual variable assignment.
#[derive(Debug, Default, Clone)]
pub struct VariableAssignment {
    assignment: BoundedMap<Variable, Sign>,
}

impl VariableAssignment {
    /// Returns the number of registered variables.
    pub fn len(&self) -> usize {
        self.assignment.capacity()
    }

    /// Returns the number of assigned variables.
    pub fn len_assigned(&self) -> usize {
        self.assignment.len()
    }

    /// Returns `true` if the assignment is complete.
    pub fn is_complete(&self) -> bool {
        self.len() == self.len_assigned()
    }

    /// Returns an iterator yielding shared references to the variable assignments.
    ///
    /// # Note
    ///
    /// Variables that have not been assigned, yet will not be yielded.
    pub fn iter(&self) -> bounded_map::Iter<Variable, Sign> {
        self.assignment.iter()
    }

    /// Registers the given number of additional variables.
    ///
    /// # Errors
    ///
    /// If the number of total variables is out of supported bounds.
    pub fn register_new_variables(&mut self, new_variables: usize) {
        let new_len = self.len() + new_variables;
        self.assignment.resize_capacity(new_len);
    }

    /// Returns the assignment for the given variable.
    ///
    /// # Panics
    ///
    /// If the variable is invalid and cannot be resolved.
    pub fn get(&self, variable: Variable) -> Option<Sign> {
        self.assignment
            .get(variable)
            .expect("encountered unexpected invalid variable")
            .copied()
    }

    /// Returns `true` if the given literal is satisfied under the current assignment.
    ///
    /// Returns `None` if the assignment is indeterminate.
    ///
    /// # Panics
    ///
    /// If the variable is invalid and cannot be resolved.
    pub fn is_satisfied(&self, literal: Literal) -> Option<bool> {
        self.get(literal.variable())
            .map(Sign::into_bool)
            .map(|assignment| {
                literal.is_positive() && assignment
                    || literal.is_negative() && !assignment
            })
    }

    /// Returns `true` if the given literal is conflicting with the current assignment.
    ///
    /// Returns `None` if the assignment is indeterminate.
    ///
    /// # Panics
    ///
    /// If the variable is invalid and cannot be resolved.
    pub fn is_conflicting(&self, literal: Literal) -> Option<bool> {
        self.is_satisfied(literal).map(|is_satisfied| !is_satisfied)
    }

    /// Updates the assignment of the variable.
    ///
    /// # Panics
    ///
    /// - If the variable is invalid and cannot be resolved.
    /// - If the variable has already been assigned.
    pub fn assign(&mut self, variable: Variable, assignment: Sign) {
        let old_assignment = self
            .assignment
            .insert(variable, assignment)
            .expect("encountered unexpected invalid variable");
        assert!(old_assignment.is_none());
    }

    /// Unassigns the given variable assignment.
    ///
    /// # Panics
    ///
    /// - If the variable is invalid and cannot be resolved.
    /// - If the variable has already been unassigned.
    pub fn unassign(&mut self, variable: Variable) {
        let old_assignment = self
            .assignment
            .take(variable)
            .expect("encountered unexpected invalid variable");
        assert!(old_assignment.is_some());
    }
}

/// The database combining everything that is realted to variable assignment.
///
/// This holds and organizes data flows through:
///
/// - Variable assignment
/// - Decision trail
/// - 2-watched literals
/// - Propagation queue
#[derive(Debug, Default, Clone)]
pub struct Assignment {
    num_variables: usize,
    trail: Trail,
    assignments: VariableAssignment,
    watchers: WatchList,
}

impl Assignment {
    /// Initializes the watchers of the assignment given the clause database.
    ///
    /// # Errors
    ///
    /// If the initialization has already taken place.
    pub fn initialize_watchers(&mut self, clause: ClauseRef) {
        let clause_id = clause.id();
        let fst = clause.first();
        let snd = clause.second();
        self.watchers.register_for_lit(!fst, snd, clause_id);
        self.watchers.register_for_lit(!snd, fst, clause_id);
    }

    /// Returns a view into the assignment.
    pub fn variable_assignment(&self) -> &VariableAssignment {
        &self.assignments
    }

    /// Returns the current number of variables.
    fn len_variables(&self) -> usize {
        self.num_variables
    }

    /// Returns the number of currently assigned variables.
    fn assigned_variables(&self) -> usize {
        self.assignments.len_assigned()
    }

    /// Returns `true` if the assignment is complete.
    fn is_complete(&self) -> bool {
        self.assignments.is_complete()
    }

    /// Registers the given number of additional variables.
    ///
    /// # Panics
    ///
    /// If the number of total variables is out of supported bounds.
    pub fn register_new_variables(&mut self, new_variables: usize) {
        self.trail.register_new_variables(new_variables);
        self.assignments.register_new_variables(new_variables);
        self.watchers.register_new_variables(new_variables);
        self.num_variables += new_variables;
    }

    /// Resets the assignment to the given decision level.
    pub fn reset_to_level(
        &mut self,
        level: DecisionLevel,
        mut inform_decider: InformDecider,
    ) {
        let Self {
            trail, assignments, ..
        } = self;
        trail.pop_to_level(level, |unassigned| {
            let variable = unassigned.variable();
            assignments.unassign(variable);
            inform_decider.restore_variable(variable)
        })
    }

    /// Enqueues a propagation literal.
    ///
    /// This does not yet perform the actual unit propagation.
    ///
    /// # Errors
    ///
    /// - If the pushed literal is in conflict with the current assignment.
    /// - If the literal has already been assigned.
    pub fn enqueue_assumption(
        &mut self,
        assumption: Literal,
    ) -> Result<(), AssignmentError> {
        self.trail.push(assumption, &mut self.assignments)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PropagationResult {
    /// Propagation led to a consistent assignment.
    Consistent,
    /// Propagation led to a conflicting assignment.
    Conflict,
}

impl PropagationResult {
    /// Returns `true` if the propagation yielded a conflict.
    pub fn is_conflict(self) -> bool {
        matches!(self, Self::Conflict)
    }
}

impl Assignment {
    /// Bumps the decision level.
    pub fn bump_decision_level(&mut self) -> DecisionLevel {
        self.trail.bump_decision_level()
    }

    /// Pops the decision level to the given level.
    ///
    /// This also unassigned all variables assigned in the given decision level.
    pub fn pop_decision_level(
        &mut self,
        level: DecisionLevel,
        mut inform_decider: InformDecider,
    ) {
        let Self {
            assignments, trail, ..
        } = self;
        trail.pop_to_level(level, |unassigned| {
            let variable = unassigned.variable();
            assignments.unassign(variable);
            inform_decider.restore_variable(variable)
        });
    }

    /// Propagates the enqueued assumptions.
    pub fn propagate(
        &mut self,
        clause_db: &mut ClauseDb,
        mut inform_decider: InformDecider,
    ) -> PropagationResult {
        let Self {
            watchers,
            assignments,
            trail,
            ..
        } = self;
        let level = trail.current_decision_level();
        while let Some(propagation_literal) = trail.pop_enqueued() {
            let result = watchers.propagate(
                propagation_literal,
                clause_db,
                assignments,
                PropagationEnqueuer::new(trail),
            );
            if result.is_conflict() {
                trail.pop_to_level(level, |unassigned| {
                    let variable = unassigned.variable();
                    assignments.unassign(variable);
                    inform_decider.restore_variable(variable)
                });
                return result
            }
        }
        PropagationResult::Consistent
    }
}

impl<'a> IntoIterator for &'a VariableAssignment {
    type Item = (Variable, Sign);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self)
    }
}

impl<'a> IntoIterator for &'a Assignment {
    type Item = (Variable, Sign);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(&self.assignments)
    }
}

pub struct Iter<'a> {
    iter: bounded_map::Iter<'a, Variable, Sign>,
}

impl<'a> Iter<'a> {
    pub fn new(assignment: &'a VariableAssignment) -> Self {
        Self {
            iter: assignment.iter(),
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Variable, Sign);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|(variable, assignment)| (variable, *assignment))
    }
}
