use super::{
    ClauseId,
    Error,
};
use crate::{
    assignment::VariableAssignment,
    Literal,
};
use core::{
    iter,
    slice,
};

/// A shared reference to a clause stored in the clause database.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClauseRef<'a> {
    /// The unique identifier of the clause.
    id: ClauseId,
    /// The literals of the clause.
    literals: &'a [Literal],
}

impl<'a> ClauseRef<'a> {
    /// Creates a new shared clause reference.
    ///
    /// # Panics
    ///
    /// If there are less than 2 literals in the literal slice.
    pub fn new(id: ClauseId, literals: &'a [Literal]) -> Result<Self, Error> {
        assert!(
            literals.len() >= 2,
            "expected at least 2 literals in a shared clause reference"
        );
        Ok(Self { id, literals })
    }

    /// Returns the identifier of the referenced clause.
    pub fn id(&self) -> ClauseId {
        self.id
    }

    /// Returns the first literal of the referenced clause.
    ///
    /// # Note
    ///
    /// This always yields a literal since this always references clauses with
    /// at least two literals.
    pub fn first(&self) -> Literal {
        self.literals[0]
    }

    /// Returns the second literal of the referenced clause.
    ///
    /// # Note
    ///
    /// This always yields a literal since this always references clauses with
    /// at least two literals.
    pub fn second(&self) -> Literal {
        self.literals[1]
    }
}

impl<'a> IntoIterator for ClauseRef<'a> {
    type Item = Literal;
    type IntoIter = iter::Copied<slice::Iter<'a, Literal>>;

    fn into_iter(self) -> Self::IntoIter {
        self.literals.iter().copied()
    }
}

/// An exclusive reference to a clause stored in the clause database.
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
    NewWatchedLiteral {
        new_watched: Literal,
        new_blocker: Literal,
    },
    /// The clause is now unit under the current assignment.
    UnitUnderAssignment(Literal),
}

impl<'a> ClauseRefMut<'a> {
    /// Creates a new exclusive clause reference for the given literal slice.
    ///
    /// # Panics
    ///
    /// If there are less than 2 literals in the literal slice.
    pub fn new(literals: &'a mut [Literal]) -> Result<Self, Error> {
        assert!(
            literals.len() >= 2,
            "expected at least 2 literals in an exclusive clause reference"
        );
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
            if assignment
                .is_satisfied(self.literals[i])
                .unwrap_or_else(|| true)
            {
                self.literals.swap(1, i);
                return PropagationResult::NewWatchedLiteral {
                    new_watched: !self.literals[1],
                    new_blocker: self.literals[0],
                }
            }
        }
        // Clause is unit under current assignment:
        PropagationResult::UnitUnderAssignment(self.literals[0])
    }
}
