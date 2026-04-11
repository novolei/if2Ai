"""
Core evaluators for Agent behavior assessment.
"""

from harness import BaseEvaluator, EvaluationResult, ExecutionResult
from typing import Dict, Any, Optional
import re


class CorrectnessEvaluator(BaseEvaluator):
    """Evaluates if the Agent output is correct."""
    
    def __init__(self, expected_output: str, match_strategy: str = "exact"):
        """
        Initialize correctness evaluator.
        
        Args:
            expected_output: The expected output
            match_strategy: "exact", "contains", or "semantic"
        """
        super().__init__("correctness")
        self.expected_output = expected_output
        self.match_strategy = match_strategy
    
    def evaluate(self, result: ExecutionResult) -> EvaluationResult:
        """Evaluate output correctness."""
        if not result.success or not result.output:
            return EvaluationResult(
                evaluator_name=self.name,
                score=0.0,
                passed=False,
                reasoning="Execution failed or no output",
            )
        
        output = result.output.strip()
        expected = self.expected_output.strip()
        
        if self.match_strategy == "exact":
            passed = output == expected
        elif self.match_strategy == "contains":
            passed = expected.lower() in output.lower()
        elif self.match_strategy == "semantic":
            # Simple semantic check: both contain key words
            passed = self._semantic_match(output, expected)
        else:
            raise ValueError(f"Unknown match strategy: {self.match_strategy}")
        
        score = 1.0 if passed else 0.0
        
        return EvaluationResult(
            evaluator_name=self.name,
            score=score,
            passed=passed,
            details={
                "expected": expected,
                "actual": output,
                "match_strategy": self.match_strategy,
            },
            reasoning=f"Output {'matches' if passed else 'does not match'} expected value",
        )
    
    @staticmethod
    def _semantic_match(output: str, expected: str) -> bool:
        """Check if outputs are semantically similar."""
        # Extract keywords and normalize
        output_words = set(re.findall(r'\w+', output.lower()))
        expected_words = set(re.findall(r'\w+', expected.lower()))
        
        # Check overlap
        overlap = output_words & expected_words
        return len(overlap) > 0


class BehaviorEvaluator(BaseEvaluator):
    """Evaluates if the Agent exhibited expected behavior."""
    
    def __init__(self, tools_used: Optional[list] = None, 
                 max_tool_calls: Optional[int] = None,
                 response_contains: Optional[list] = None):
        """
        Initialize behavior evaluator.
        
        Args:
            tools_used: List of tool names that should be used
            max_tool_calls: Maximum number of tool calls allowed
            response_contains: List of strings that response should contain
        """
        super().__init__("behavior")
        self.tools_used = tools_used or []
        self.max_tool_calls = max_tool_calls
        self.response_contains = response_contains or []
    
    def evaluate(self, result: ExecutionResult) -> EvaluationResult:
        """Evaluate Agent behavior."""
        if not result.success:
            return EvaluationResult(
                evaluator_name=self.name,
                score=0.0,
                passed=False,
                reasoning="Execution failed",
            )
        
        issues = []
        
        # Check tool usage
        if self.tools_used:
            # Count tools used from events
            used_tools = set()
            for event in result.events:
                if event.type.value == "tool_called":
                    used_tools.add(event.details.get("tool"))
            
            missing_tools = set(self.tools_used) - used_tools
            if missing_tools:
                issues.append(f"Tools not used: {missing_tools}")
        
        # Check tool call count
        if self.max_tool_calls is not None:
            tool_calls = result.metrics.tool_calls
            if tool_calls > self.max_tool_calls:
                issues.append(f"Too many tool calls: {tool_calls} > {self.max_tool_calls}")
        
        # Check response content
        if self.response_contains and result.output:
            output_lower = result.output.lower()
            missing_content = [
                s for s in self.response_contains 
                if s.lower() not in output_lower
            ]
            if missing_content:
                issues.append(f"Response missing: {missing_content}")
        
        passed = len(issues) == 0
        score = 1.0 if passed else max(0.0, 1.0 - len(issues) * 0.25)
        
        return EvaluationResult(
            evaluator_name=self.name,
            score=score,
            passed=passed,
            details={
                "tools_used": self.tools_used,
                "max_tool_calls": self.max_tool_calls,
                "response_contains": self.response_contains,
                "issues": issues,
            },
            reasoning=", ".join(issues) if issues else "Behavior matches expectations",
        )


class PerformanceEvaluator(BaseEvaluator):
    """Evaluates performance metrics."""
    
    def __init__(self, max_tokens: Optional[int] = None,
                 max_duration: Optional[float] = None,
                 max_tool_calls: Optional[int] = None):
        """
        Initialize performance evaluator.
        
        Args:
            max_tokens: Maximum tokens allowed
            max_duration: Maximum duration in seconds
            max_tool_calls: Maximum tool calls allowed
        """
        super().__init__("performance")
        self.max_tokens = max_tokens
        self.max_duration = max_duration
        self.max_tool_calls = max_tool_calls
    
    def evaluate(self, result: ExecutionResult) -> EvaluationResult:
        """Evaluate performance metrics."""
        issues = []
        
        # Check token usage
        if self.max_tokens and result.metrics.total_tokens > self.max_tokens:
            issues.append(f"Token usage exceeded: "
                         f"{result.metrics.total_tokens} > {self.max_tokens}")
        
        # Check duration
        if self.max_duration and result.metrics.duration_seconds > self.max_duration:
            issues.append(f"Duration exceeded: "
                         f"{result.metrics.duration_seconds}s > {self.max_duration}s")
        
        # Check tool calls
        if self.max_tool_calls and result.metrics.tool_calls > self.max_tool_calls:
            issues.append(f"Tool calls exceeded: "
                         f"{result.metrics.tool_calls} > {self.max_tool_calls}")
        
        passed = len(issues) == 0
        score = 1.0 if passed else max(0.0, 1.0 - len(issues) * 0.25)
        
        return EvaluationResult(
            evaluator_name=self.name,
            score=score,
            passed=passed,
            details={
                "max_tokens": self.max_tokens,
                "max_duration": self.max_duration,
                "max_tool_calls": self.max_tool_calls,
                "actual_tokens": result.metrics.total_tokens,
                "actual_duration": result.metrics.duration_seconds,
                "actual_tool_calls": result.metrics.tool_calls,
                "issues": issues,
            },
            reasoning=", ".join(issues) if issues else "Performance within limits",
        )


__all__ = [
    "CorrectnessEvaluator",
    "BehaviorEvaluator",
    "PerformanceEvaluator",
]
