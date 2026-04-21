# Execution Mode Policy Routing Gap — 框架深析

当前 If2Ai 最大的问题之一，是“会判断”不等于“会分流”。

这类 gap 会制造一种很危险的产品幻觉：

- UI 看起来更聪明了
- diagnostics 更多了
- 但行为仍和以前一样

UClaw 的策略层之所以重要，不是因为它能生成一份 decision，而是因为它决定真实执行路径。

If2Ai 若不把这件事做完，execution mode 就永远更像 annotation，而不是 runtime truth。

