// 阅读区页边距纯逻辑模块：不依赖真实 DOM（浏览器走 window.StillwriteMarginControl，
// Node 测试走 module.exports）。
//
// 状态只有两态，单一数据源 = localStorage 键 stillwrite.markdownPaddingX：
//   null = auto（未设置该键） → 中间控件显示 AUTO，不加 body.margin-fixed
//   px   = fixed（用户设过固定值） → 显示当前 px 数值，加 body.margin-fixed
// 中间数值是可点击的显式状态控件：在 fixed 状态点击 = 删除存储键，立即恢复 auto。
// − / ＋ 在 auto 状态下先以 MARGIN_AUTO_SEED 进入 fixed，再正常步进。
// 纯展示偏好（AGENTS.md 2.1 允许 localStorage），不进入任何 durable state/DB。

(function exposeMarginControl(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteMarginControl = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	const MARGIN_STORAGE_KEY = "stillwrite.markdownPaddingX";
	const MARGIN_MIN = 16;
	const MARGIN_MAX = 480;
	const MARGIN_STEP = 8;
	// 从 auto 首次按 − / ＋ 的起步值：取写/双栏 clamp 在常见窗宽下的典型生效量，
	// 保证首击方向与按钮语义一致（− 变窄、＋ 变宽），而不是从极值跳变。
	const MARGIN_AUTO_SEED = 72;

	// 读取存储中的固定值；键缺失或非法 → null（auto）。
	// 已有用户的 localStorage 固定值原样保留（不迁移、不改写）。
	function storedValue(storage) {
		const raw = storage.getItem(MARGIN_STORAGE_KEY);
		if (raw === null) return null;
		const value = parseInt(raw, 10);
		return Number.isNaN(value) ? null : value;
	}

	function clamp(value) {
		return Math.max(MARGIN_MIN, Math.min(MARGIN_MAX, value));
	}

	// 从当前值向 direction（−1 / ＋1）步进；auto（null）时先用 seed 进入 fixed。
	function steppedValue(value, direction) {
		return clamp((value ?? MARGIN_AUTO_SEED) + direction * MARGIN_STEP);
	}

	// 中间控件显示文案：auto → "AUTO"；fixed → 当前 px 数值。
	function displayLabel(value) {
		return value === null ? "AUTO" : String(value);
	}

	// 状态 → DOM 投影声明（app.js 只按此应用，不重算逻辑）。
	// 规则：auto 绝不添加 body.margin-fixed（读单模式保持居中版心）；
	// fixed 添加 body.margin-fixed 并写 --markdown-padding-x / --editor-padding-x。
	function project(value) {
		if (value === null) {
			return {
				value: null,
				label: "AUTO",
				setCss: [],
				removeCss: ["--markdown-padding-x", "--editor-padding-x"],
				addClass: [],
				removeClass: ["margin-fixed"],
			};
		}
		return {
			value,
			label: String(value),
			setCss: [
				["--markdown-padding-x", `${value}px`],
				["--editor-padding-x", `${value}px`],
			],
			removeCss: [],
			addClass: ["margin-fixed"],
			removeClass: [],
		};
	}

	// 依赖注入方式创建实例：storage + DOM 写入回调（apply 接收 project 的返回值）。
	// 所有命令在同一份状态上操作；测试可注入假 storage / 记录型 apply。
	function create({ storage, apply: writeDom }) {
		let value = storedValue(storage);

		function sync() {
			writeDom(project(value));
			return value;
		}

		// 设定固定值并持久化（− / ＋ 最终都走到这里进入 fixed）。
		function setFixed(target) {
			value = clamp(target);
			storage.setItem(MARGIN_STORAGE_KEY, String(value));
			return sync();
		}

		function step(direction) {
			return setFixed(steppedValue(value, direction));
		}

		// 点击中间数值：立即删除存储键并恢复 auto，同步刷新 DOM。
		function resetToAuto() {
			value = null;
			storage.removeItem(MARGIN_STORAGE_KEY);
			return sync();
		}

		return {
			get value() {
				return value;
			},
			sync,
			setFixed,
			step,
			resetToAuto,
		};
	}

	return {
		MARGIN_STORAGE_KEY,
		MARGIN_MIN,
		MARGIN_MAX,
		MARGIN_STEP,
		MARGIN_AUTO_SEED,
		storedValue,
		clamp,
		steppedValue,
		displayLabel,
		project,
		create,
	};
});
