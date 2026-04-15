//! Query intent classification and retrieval weights
//!
//! Classifies user queries into intent categories, each with different
//! memory retrieval weight distributions across semantic, episodic,
//! and working memory layers.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Intent categories for routing memory retrieval
///
/// Each variant carries different retrieval weights that determine
/// how much each memory layer contributes to the final result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum QueryIntent {
    /// Code-related queries: semantic=0.5, episodic=0.3, working=0.2
    Code,
    /// Task-related queries: semantic=0.4, episodic=0.4, working=0.2
    Task,
    /// Factual queries: semantic=0.6, episodic=0.2, working=0.2
    Fact,
    /// Person-related queries: semantic=0.3, episodic=0.5, working=0.2
    Person,
    /// General queries: semantic=0.3, episodic=0.3, working=0.4
    #[default]
    General,
}

impl QueryIntent {
    /// Returns retrieval weights as (semantic, episodic, working)
    #[must_use]
    pub fn retrieval_weights(&self) -> (f32, f32, f32) {
        match self {
            QueryIntent::Code => (0.5, 0.3, 0.2),
            QueryIntent::Task => (0.4, 0.4, 0.2),
            QueryIntent::Fact => (0.6, 0.2, 0.2),
            QueryIntent::Person => (0.3, 0.5, 0.2),
            QueryIntent::General => (0.3, 0.3, 0.4),
        }
    }
}

/// Retrieve weights as a dedicated struct for configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetrievalWeights {
    pub semantic: f32,
    pub episodic: f32,
    pub working: f32,
}

impl From<QueryIntent> for RetrievalWeights {
    fn from(intent: QueryIntent) -> Self {
        let (semantic, episodic, working) = intent.retrieval_weights();
        Self {
            semantic,
            episodic,
            working,
        }
    }
}

/// Classify a query into an intent category using keyword matching
///
/// This is a rule-based classifier. Future work (P4+) may add
/// ML-based intent classification.
#[must_use]
pub fn classify_intent(query: &str) -> QueryIntent {
    let q = query.to_lowercase();

    // Code intent
    if q.contains("how do i")
        || q.contains("fix")
        || q.contains("bug")
        || q.contains("error")
        || q.contains("implement")
        || q.contains("code")
        || q.contains("debug")
    {
        return QueryIntent::Code;
    }

    // Task intent
    if q.contains("remember")
        || q.contains("did we")
        || q.contains("did i")
        || q.contains("task")
        || q.contains("what did")
        || q.contains("status")
        || q.contains("progress")
    {
        return QueryIntent::Task;
    }

    // Person intent
    if q.starts_with("who")
        || q.contains("person")
        || q.contains("author")
        || q.contains("created")
        || q.contains("team")
    {
        return QueryIntent::Person;
    }

    // Fact intent
    if q.starts_with("what is")
        || q.starts_with("what are")
        || q.starts_with("what's")
        || q.contains("fact")
        || q.contains("definition")
        || q.contains("mean")
        || q.contains("explain")
    {
        return QueryIntent::Fact;
    }

    // Default
    QueryIntent::General
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_intent_keywords() {
        let queries = [
            "How do I fix this?",
            "There's a bug in the code",
            "Debug the error",
            "Implement feature",
        ];
        for q in queries {
            assert_eq!(classify_intent(q), QueryIntent::Code, "query: {q}");
        }
    }

    #[test]
    fn task_intent_keywords() {
        let queries = [
            "Remember this",
            "Did we finish the task?",
            "What did we do yesterday?",
            "Task status",
        ];
        for q in queries {
            assert_eq!(classify_intent(q), QueryIntent::Task, "query: {q}");
        }
    }

    #[test]
    fn person_intent_keywords() {
        let queries = [
            "Who created this?",
            "The person who wrote this",
            "Author of the file",
        ];
        for q in queries {
            assert_eq!(classify_intent(q), QueryIntent::Person, "query: {q}");
        }
    }

    #[test]
    fn fact_intent_keywords() {
        let queries = [
            "What is the meaning of life?",
            "What are the rules?",
            "Explain the concept",
        ];
        for q in queries {
            assert_eq!(classify_intent(q), QueryIntent::Fact, "query: {q}");
        }
    }

    #[test]
    fn general_default() {
        let queries = ["Hello", "Random thought", "Some text"];
        for q in queries {
            assert_eq!(classify_intent(q), QueryIntent::General, "query: {q}");
        }
    }

    #[test]
    fn retrieval_weights_code() {
        let (sem, epi, work) = QueryIntent::Code.retrieval_weights();
        assert!((sem - 0.5).abs() < 0.001);
        assert!((epi - 0.3).abs() < 0.001);
        assert!((work - 0.2).abs() < 0.001);
    }

    #[test]
    fn retrieval_weights_task() {
        let (sem, epi, work) = QueryIntent::Task.retrieval_weights();
        assert!((sem - 0.4).abs() < 0.001);
        assert!((epi - 0.4).abs() < 0.001);
        assert!((work - 0.2).abs() < 0.001);
    }

    #[test]
    fn retrieval_weights_fact() {
        let (sem, epi, work) = QueryIntent::Fact.retrieval_weights();
        assert!((sem - 0.6).abs() < 0.001);
        assert!((epi - 0.2).abs() < 0.001);
        assert!((work - 0.2).abs() < 0.001);
    }

    #[test]
    fn retrieval_weights_person() {
        let (sem, epi, work) = QueryIntent::Person.retrieval_weights();
        assert!((sem - 0.3).abs() < 0.001);
        assert!((epi - 0.5).abs() < 0.001);
        assert!((work - 0.2).abs() < 0.001);
    }

    #[test]
    fn retrieval_weights_general() {
        let (sem, epi, work) = QueryIntent::General.retrieval_weights();
        assert!((sem - 0.3).abs() < 0.001);
        assert!((epi - 0.3).abs() < 0.001);
        assert!((work - 0.4).abs() < 0.001);
    }

    #[test]
    fn weights_sum_to_one() {
        for intent in [
            QueryIntent::Code,
            QueryIntent::Task,
            QueryIntent::Fact,
            QueryIntent::Person,
            QueryIntent::General,
        ] {
            let (sem, epi, work) = intent.retrieval_weights();
            let sum = sem + epi + work;
            assert!((sum - 1.0).abs() < 0.001, "{intent:?} weights sum to {sum}");
        }
    }

    #[test]
    fn from_query_intent_to_weights() {
        let intent = QueryIntent::Code;
        let weights: RetrievalWeights = intent.into();
        assert!((weights.semantic - 0.5).abs() < 0.001);
        assert!((weights.episodic - 0.3).abs() < 0.001);
        assert!((weights.working - 0.2).abs() < 0.001);
    }

    #[test]
    fn query_intent_default() {
        assert_eq!(QueryIntent::default(), QueryIntent::General);
    }

    #[test]
    fn case_insensitive_classification() {
        assert_eq!(classify_intent("HOW DO I FIX THIS"), QueryIntent::Code);
        assert_eq!(classify_intent("WHO Created This"), QueryIntent::Person);
        assert_eq!(classify_intent("WHAT IS the meaning"), QueryIntent::Fact);
    }
}
