export default {
	defaultLayout: "master-stack",
	layoutRules: [
		{
			index: 0,
			layout: "master-stack"
		},
		{
			index: 1,
			layout: "master-stack"
		},
		{
			index: 2,
			layout: "master-stack"
		},
		{
			index: 3,
			layout: "master-stack"
		},
		{
			index: 4,
			layout: "testing"
		},
		{
			index: 5,
			layout: "primary-stack"
		},
		{
			index: 6,
			layout: "primary-stack"
		},
		{
			index: 7,
			layout: "primary-stack"
		},
		{
			index: 8,
			layout: "random"
		},
		{
			monitor: "eDP-1",
			layout: "master-stack"
		}
	],
	resize: {
		stepPx: 96,
		minBranchSizePx: 120
	}
};
