"""
Base classes and interfaces for If2Ai Harness Framework.

Provides core abstractions for running, evaluating, and comparing Agent behavior.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Union
from enum import Enum
import json
from datetime import datetime
import uuid


class ExecutionEventType(Enum):
    """Types of events that can occur during Agent execution."""
    INITIALIZED = "initialized"
    PROMPT_SENT = "prompt_sent"
    TOOL_CALLED = "tool_called"
    TOOL_RESULT = "tool_result"
    LLM_CALLED = "llm_called"
    LLM_RESPONSE = "llm_response"
    CONTEXT_UPDATED = "context_updated"
    COMPLETION = "completion"
    ERROR = "error"
    TIMEOUT = "timeout"


@dataclass
class ExecutionEvent:
    """Represents a single event during Agent execution."""
    type: ExecutionEventType
    timestamp: datetime
    details: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": self.type.value,
            "timestamp": self.timestamp.isoformat(),
            "details": self.details,
        }


@dataclass
class ExecutionMetrics:
    """Metrics collected during Agent execution."""
    total_tokens: int = 0
    prompt_tokens: int = 0
    completion_tokens: int = 0
    duration_seconds: float = 0.0
    tool_calls: int = 0
    error_count: int = 0
    errors: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "total_tokens": self.total_tokens,
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "duration_seconds": self.duration_seconds,
            "tool_calls": self.tool_calls,
            "error_count": self.error_count,
            "errors": self.errors,
        }


@dataclass
class ExecutionResult:
    """Result of a single Agent execution."""
    run_id: str
    prompt: str
    output: Optional[str] = None
    success: bool = True
    events: List[ExecutionEvent] = field(default_factory=list)
    metrics: ExecutionMetrics = field(default_factory=ExecutionMetrics)
    error: Optional[str] = None
    context_snapshot: Optional[Dict[str, Any]] = None
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "run_id": self.run_id,
            "prompt": self.prompt,
            "output": self.output,
            "success": self.success,
            "events": [e.to_dict() for e in self.events],
            "metrics": self.metrics.to_dict(),
            "error": self.error,
        }


class BaseEvaluator(ABC):
    """Base class for all evaluators."""
    
    def __init__(self, name: str, **config):
        self.name = name
        self.config = config
    
    @abstractmethod
    def evaluate(self, result: ExecutionResult) -> "EvaluationResult":
        """
        Evaluate the execution result.
        
        Args:
            result: The execution result to evaluate
            
        Returns:
            EvaluationResult with score and details
        """
        pass


@dataclass
class EvaluationResult:
    """Result of evaluating an Agent execution."""
    evaluator_name: str
    score: float  # 0.0 to 1.0
    passed: bool
    details: Dict[str, Any] = field(default_factory=dict)
    reasoning: str = ""
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "evaluator": self.evaluator_name,
            "score": self.score,
            "passed": self.passed,
            "details": self.details,
            "reasoning": self.reasoning,
        }


@dataclass
class TestCase:
    """Definition of a single test case."""
    name: str
    description: str
    prompt: str
    expected_output: str
    evaluators: List[str] = field(default_factory=list)
    tags: List[str] = field(default_factory=list)
    timeout_seconds: float = 30.0
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "description": self.description,
            "prompt": self.prompt,
            "expected_output": self.expected_output,
            "evaluators": self.evaluators,
            "tags": self.tags,
            "timeout": self.timeout_seconds,
        }


@dataclass
class TestRunResult:
    """Result of running a single test case."""
    test_case: TestCase
    execution: ExecutionResult
    evaluations: List[EvaluationResult] = field(default_factory=list)
    
    @property
    def passed(self) -> bool:
        """Test passes if all evaluations pass."""
        return all(e.passed for e in self.evaluations)
    
    @property
    def overall_score(self) -> float:
        """Average score across all evaluators."""
        if not self.evaluations:
            return 0.0
        return sum(e.score for e in self.evaluations) / len(self.evaluations)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "test": self.test_case.to_dict(),
            "execution": self.execution.to_dict(),
            "evaluations": [e.to_dict() for e in self.evaluations],
            "passed": self.passed,
            "overall_score": self.overall_score,
        }


@dataclass
class TestSuiteResult:
    """Result of running a complete test suite."""
    suite_name: str
    timestamp: datetime
    test_results: List[TestRunResult] = field(default_factory=list)
    
    @property
    def total_tests(self) -> int:
        return len(self.test_results)
    
    @property
    def passed_tests(self) -> int:
        return sum(1 for r in self.test_results if r.passed)
    
    @property
    def failed_tests(self) -> int:
        return self.total_tests - self.passed_tests
    
    @property
    def pass_rate(self) -> float:
        if self.total_tests == 0:
            return 0.0
        return self.passed_tests / self.total_tests
    
    @property
    def average_score(self) -> float:
        if self.total_tests == 0:
            return 0.0
        return sum(r.overall_score for r in self.test_results) / self.total_tests
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "suite": self.suite_name,
            "timestamp": self.timestamp.isoformat(),
            "stats": {
                "total": self.total_tests,
                "passed": self.passed_tests,
                "failed": self.failed_tests,
                "pass_rate": self.pass_rate,
                "average_score": self.average_score,
            },
            "results": [r.to_dict() for r in self.test_results],
        }


class BaseRunner(ABC):
    """Base class for test runners."""
    
    def __init__(self, name: str, **config):
        self.name = name
        self.config = config
        self.evaluators: Dict[str, BaseEvaluator] = {}
    
    def register_evaluator(self, evaluator: BaseEvaluator) -> None:
        """Register an evaluator."""
        self.evaluators[evaluator.name] = evaluator
    
    @abstractmethod
    async def run(self, test_case: TestCase) -> ExecutionResult:
        """
        Run a single test case.
        
        Args:
            test_case: The test case to run
            
        Returns:
            ExecutionResult with output and metrics
        """
        pass
    
    async def evaluate(
        self, 
        result: ExecutionResult, 
        evaluator_names: Optional[List[str]] = None
    ) -> List[EvaluationResult]:
        """
        Evaluate an execution result using specified evaluators.
        
        Args:
            result: The execution result to evaluate
            evaluator_names: List of evaluator names to use (or all if None)
            
        Returns:
            List of evaluation results
        """
        names = evaluator_names or list(self.evaluators.keys())
        results = []
        
        for name in names:
            if name not in self.evaluators:
                raise ValueError(f"Unknown evaluator: {name}")
            
            evaluator = self.evaluators[name]
            eval_result = evaluator.evaluate(result)
            results.append(eval_result)
        
        return results
    
    async def run_test(
        self, 
        test_case: TestCase
    ) -> TestRunResult:
        """
        Run a test case and evaluate results.
        
        Args:
            test_case: The test case to run
            
        Returns:
            TestRunResult with execution and evaluations
        """
        # Execute the test
        execution = await self.run(test_case)
        
        # Evaluate the results
        evaluator_names = test_case.evaluators or list(self.evaluators.keys())
        evaluations = await self.evaluate(execution, evaluator_names)
        
        return TestRunResult(
            test_case=test_case,
            execution=execution,
            evaluations=evaluations,
        )
    
    async def run_suite(
        self, 
        test_cases: List[TestCase]
    ) -> TestSuiteResult:
        """
        Run a complete test suite.
        
        Args:
            test_cases: List of test cases to run
            
        Returns:
            TestSuiteResult with all results
        """
        results = []
        
        for test_case in test_cases:
            result = await self.run_test(test_case)
            results.append(result)
        
        return TestSuiteResult(
            suite_name=self.name,
            timestamp=datetime.now(),
            test_results=results,
        )


def create_execution_result(
    prompt: str,
    output: str,
    success: bool = True,
    error: Optional[str] = None,
    metrics: Optional[ExecutionMetrics] = None,
) -> ExecutionResult:
    """Factory function to create an ExecutionResult."""
    return ExecutionResult(
        run_id=f"run-{datetime.now().isoformat()}-{uuid.uuid4().hex[:8]}",
        prompt=prompt,
        output=output,
        success=success,
        error=error,
        metrics=metrics or ExecutionMetrics(),
    )


__all__ = [
    "ExecutionEventType",
    "ExecutionEvent",
    "ExecutionMetrics",
    "ExecutionResult",
    "BaseEvaluator",
    "EvaluationResult",
    "TestCase",
    "TestRunResult",
    "TestSuiteResult",
    "BaseRunner",
    "create_execution_result",
]
