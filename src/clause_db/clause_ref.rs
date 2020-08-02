use super::Error;
use crate::{
    assignment2::VariableAssignment,
    Literal,
};
use super::ClauseId;
use core::{
    iter,
    slice,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClauseRef<'a> {
    id: ClauseId,
    literals: &'a [Literal],
}

impl<'a> ClauseRef<'a> {
    /// Creates a new shared clause reference.
    pub fn new(id: ClauseId, literals: &'a [Literal]) -> Result<Self, Error> {
        debug_assert!(!literals.is_empty());
        Ok(Self { id, literals })
    }

    /// Returns the identifier of the referenced clause.
    pub fn id(&self) -> ClauseId {
        self.id
    }
}

impl<'a> IntoIterator for ClauseRef<'a> {
    type Item = Literal;
    type IntoIter = iter::Copied<slice::Iter<'a, Literal>>;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.iter().copied()
    }
}

#[derive(Debug)]
pub struct ClauseRefMut<'a> {
    literals: &'a mut [Literal],
}

/// Result returned from clause local propagation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PropagationResult {
    /// The clause is already satisfied under the current assignment.
    AlreadySatisfied,
    /// The clause chose a new watched literal.
    NewWatchedLiteral(Literal),
    /// The clause is now unit under the current assignment.
    UnitUnderAssignment(Literal),
}

impl<'a> ClauseRefMut<'a> {
    /// Creates a new exclusive clause reference for the given literal slice.
    ///
    /// # Panics
    ///
    /// If the given literal slice is empty.
    pub fn new(literals: &'a mut [Literal]) -> Result<Self, Error> {
        debug_assert!(!literals.is_empty());
        Ok(Self { literals })
    }

    pub fn propagate(
        &mut self,
        propagated_lit: Literal,
        assignment: &VariableAssignment,
    ) -> PropagationResult {
        // Make sure the false literal is in the second [1] position.
        if self.literals[0] == !propagated_lit {
            self.literals.swap(0, 1);
        }
        // If 0-th watch is true, then clause is already satisfied.
        if assignment
            .is_satisfied(self.literals[0])
            .unwrap_or_else(|| false)
        {
            return PropagationResult::AlreadySatisfied
        }
        // Look for new literal to watch:
        for i in 2..self.literals.len() {
            if !assignment
                .is_satisfied(self.literals[0])
                .unwrap_or_else(|| false)
            {
                self.literals.swap(1, i);
                return PropagationResult::NewWatchedLiteral(self.literals[1])
            }
        }
        // Clause is unit under current assignment:
        PropagationResult::UnitUnderAssignment(self.literals[0])
    }
}
