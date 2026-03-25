⸻

Semantic Input & Token Graph Expression System（草稿 v0.1）

1. 核心定义（Core Idea）

本系统不是输入法，而是：

一种从「语义种子（seed）」出发，通过图结构构建表达，并在语法闭合后生成多风格输出的表达构建机制。

核心目标：
	•	降低表达成本（不再手写完整句子/命令）
	•	提供结构化表达路径（可组合、可验证）
	•	将“写”转化为“构建”

⸻

2. 基本流程（Execution Flow）

Step 1：Seed 输入

用户输入一个最小语义单元：

deploy
summarize
monitor cpu


⸻

Step 2：语义展开（Semantic Expansion）

系统基于 seed 构建候选图（Token Graph）：

示例：

deploy
 ├── target: staging / production
 ├── strategy: rolling / recreate
 ├── notify: slack / email


⸻

Step 3：路径选择与组合（Path Composition）

用户通过选择节点构建表达路径：

deploy → production → rolling → notify:slack


⸻

Step 4：语法闭合（Syntax Closure）

系统判断表达是否满足：
	•	必要参数是否齐全
	•	结构是否合法
	•	是否可执行 / 可生成

若未闭合 → 提示缺失节点

⸻

Step 5：表达生成（Expression Rendering）

在闭合后，系统生成多种输出形式：

形式A：结构化表达（IR / AST）

Deploy {
  target: production,
  strategy: rolling,
  notify: slack
}

形式B：可执行命令

deploy --env=prod --strategy=rolling --notify=slack

形式C：自然语言（多风格）
	•	正式：Deploy to production using rolling strategy and notify via Slack.
	•	简洁：Rolling deploy to prod + Slack notify.
	•	解释型：Perform a rolling deployment to production and send notification to Slack.

⸻

3. 核心结构（Core Model）

3.1 Token Graph
	•	节点：语义单元（token）
	•	边：可组合关系
	•	类型：
	•	必选节点（required）
	•	可选节点（optional）
	•	分支节点（branch）
	•	约束节点（constraint）

⸻

3.2 语法规则（Grammar Constraints）

定义：
	•	必要字段
	•	排他关系（A or B）
	•	依赖关系（A requires B）

⸻

3.3 语义闭合条件（Closure）

表达必须满足：
	•	所有 required 节点已选
	•	不存在冲突路径
	•	满足依赖约束

⸻

4. 系统定位（Positioning）

本系统不是：
	•	❌ 输入法（IME）
	•	❌ LLM prompt 工具
	•	❌ 自动生成器

本系统是：

表达构建层（Expression Construction Layer）

⸻

5. 适用场景（Valid Domains）

✔ 高结构化表达：
	•	CLI / DevOps 操作
	•	系统控制指令
	•	API 调用构建
	•	配置生成
	•	规则定义（如网络/安全）

⸻

❌ 不适用：
	•	自由聊天
	•	情绪表达
	•	高个性文本创作

⸻

6. 与 LLM 的关系

LLM 不参与结构构建，仅用于：
	•	候选节点排序
	•	表达风格生成
	•	语义解释

⸻

核心原则：

结构由系统保证，表达由 LLM增强

⸻


