import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip } from "recharts";

const DATA = [
  { month: "Jan", budgeted: 180, actual: 160 },
  { month: "Feb", budgeted: 220, actual: 210 },
  { month: "Mar", budgeted: 190, actual: 205 },
  { month: "Apr", budgeted: 240, actual: 230 },
  { month: "May", budgeted: 280, actual: 265 },
  { month: "Jun", budgeted: 260, actual: 245 },
];

export default function BudgetStatus() {
  return (
    <div data-cmp="BudgetStatus" className="bg-surface border border-border rounded-xl shadow-custom">
      <div className="px-4 py-3.5 border-b border-border">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="font-serif text-[15px] font-semibold text-foreground">
              Budget Overview
            </h2>
            <p className="text-muted-foreground text-[10px] font-sans mt-0.5">
              YTD — $2.4M of $3.1M allocated
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-sm bg-charcoal" />
              <span className="text-muted-foreground text-[10px] font-sans">Budgeted</span>
            </div>
            <div className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-sm bg-terracotta" />
              <span className="text-muted-foreground text-[10px] font-sans">Actual</span>
            </div>
          </div>
        </div>
      </div>
      <div className="px-4 py-3">
        <ResponsiveContainer width="100%" height={100}>
          <BarChart data={DATA} barSize={10} barGap={3}>
            <XAxis
              dataKey="month"
              axisLine={false}
              tickLine={false}
              tick={{ fontSize: 10, fill: "#8a8379", fontFamily: "Inter" }}
            />
            <YAxis hide />
            <Tooltip
              contentStyle={{
                background: "#faf9f7",
                border: "1px solid #ddd8d0",
                borderRadius: "6px",
                fontSize: "11px",
                fontFamily: "Inter",
              }}
              formatter={(v: number) => [`$${v}K`, ""]}
            />
            <Bar dataKey="budgeted" fill="#2c2825" radius={[2, 2, 0, 0]} />
            <Bar dataKey="actual" fill="#b85c38" radius={[2, 2, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
