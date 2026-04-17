import { Calendar, Video, MapPin } from "lucide-react";

type Review = {
  id: string;
  title: string;
  project: string;
  date: string;
  time: string;
  type: "video" | "in-person";
  client: string;
  initials: string;
};

const REVIEWS: Review[] = [
  { id: "1", title: "Design Concept Presentation", project: "Kensington Penthouse", date: "Jun 17", time: "10:00", type: "video", client: "A. Harrington", initials: "AH" },
  { id: "2", title: "Material Selection Review", project: "Maison Rivière", date: "Jun 19", time: "14:30", type: "in-person", client: "F. Beaumont", initials: "FB" },
  { id: "3", title: "Final Proposal Sign-off", project: "Aldgate Loft", date: "Jun 21", time: "11:00", type: "video", client: "N. Okafor", initials: "NO" },
];

export default function UpcomingReviews() {
  return (
    <div data-cmp="UpcomingReviews" className="bg-surface border border-border rounded-xl shadow-custom">
      <div className="px-4 py-3.5 border-b border-border flex items-center gap-2">
        <Calendar size={13} className="text-gold" />
        <h2 className="font-serif text-[15px] font-semibold text-foreground">
          Upcoming Reviews
        </h2>
      </div>
      <div className="divide-y divide-border">
        {REVIEWS.map((r) => (
          <div key={r.id} className="flex items-start gap-3 px-4 py-3 hover:bg-muted/30 transition-all">
            <div className="w-9 h-9 rounded-lg bg-warm-beige-dark border border-border flex flex-col items-center justify-center shrink-0">
              <span className="text-[9px] font-sans text-muted-foreground uppercase tracking-wide leading-none">
                {r.date.split(" ")[0]}
              </span>
              <span className="text-[15px] font-serif font-semibold text-foreground leading-tight">
                {r.date.split(" ")[1]}
              </span>
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-[12px] font-sans font-semibold text-foreground truncate">
                {r.title}
              </p>
              <p className="text-muted-foreground text-[10px] font-sans truncate mt-0.5">
                {r.project} · {r.client}
              </p>
              <div className="flex items-center gap-2 mt-1">
                <span className="text-[10px] font-sans text-muted-foreground">
                  {r.time}
                </span>
                <span className={`flex items-center gap-1 text-[10px] font-sans font-medium ${
                  r.type === "video" ? "text-olive" : "text-terracotta"
                }`}>
                  {r.type === "video" ? <Video size={9} /> : <MapPin size={9} />}
                  {r.type === "video" ? "Video Call" : "In Person"}
                </span>
              </div>
            </div>
            <div className="w-6 h-6 rounded-full bg-terracotta-light flex items-center justify-center text-[9px] font-semibold text-terracotta shrink-0">
              {r.initials}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
