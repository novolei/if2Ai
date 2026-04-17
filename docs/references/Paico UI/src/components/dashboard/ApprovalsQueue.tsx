import { CheckCircle, XCircle, Clock } from "lucide-react";

type Approval = {
  id: string;
  title: string;
  project: string;
  submittedBy: string;
  age: string;
  type: string;
};

const APPROVALS: Approval[] = [
  { id: "1", title: "Concept Board v3", project: "Kensington Penthouse", submittedBy: "JL", age: "2d", type: "Moodboard" },
  { id: "2", title: "Fabric Selection Final", project: "Maison Rivière", submittedBy: "TR", age: "3d", type: "Material" },
  { id: "3", title: "Floor Plan Rev.2", project: "Aldgate Loft", submittedBy: "MR", age: "5d", type: "Drawing" },
];

export default function ApprovalsQueue() {
  return (
    <div data-cmp="ApprovalsQueue" className="bg-surface border border-border rounded-xl shadow-custom">
      <div className="px-4 py-3.5 border-b border-border flex items-center gap-2">
        <Clock size={13} className="text-gold" />
        <h2 className="font-serif text-[15px] font-semibold text-foreground">
          Pending Approvals
        </h2>
        <span className="ml-auto tag-pill bg-gold-light text-gold">
          {APPROVALS.length} pending
        </span>
      </div>
      <div className="divide-y divide-border">
        {APPROVALS.map((a) => (
          <div key={a.id} className="flex items-center gap-3 px-4 py-3">
            <div className="flex-1 min-w-0">
              <p className="text-[12px] font-sans font-semibold text-foreground truncate">
                {a.title}
              </p>
              <div className="flex items-center gap-1.5 mt-0.5">
                <span className="tag-pill">{a.type}</span>
                <span className="text-muted-foreground text-[10px] font-sans">
                  {a.project}
                </span>
                <span className="text-muted-foreground text-[10px] font-sans">
                  · {a.age} ago
                </span>
              </div>
            </div>
            <div className="flex items-center gap-1 shrink-0">
              <button className="w-6 h-6 rounded-md bg-olive-light hover:bg-olive hover:text-primary-foreground flex items-center justify-center text-olive transition-all">
                <CheckCircle size={12} />
              </button>
              <button className="w-6 h-6 rounded-md bg-terracotta-light hover:bg-terracotta hover:text-primary-foreground flex items-center justify-center text-terracotta transition-all">
                <XCircle size={12} />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
