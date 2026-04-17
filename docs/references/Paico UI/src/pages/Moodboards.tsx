import { useState } from "react";
import { Search, Plus, Heart, Share2, Tag, SlidersHorizontal } from "lucide-react";

type Board = {
  id: string;
  title: string;
  project: string;
  style: string[];
  updated: string;
  images: string[];
  liked: boolean;
};

const BOARDS: Board[] = [
  {
    id: "1",
    title: "Kensington — Concept A",
    project: "Kensington Penthouse",
    style: ["Minimalist", "Warm", "Luxury"],
    updated: "2d ago",
    liked: true,
    images: [
      "https://images.unsplash.com/photo-1600607687939-ce8a6c25118c?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1555041469-a586c61ea9bc?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1616486338812-3dadae4b4ace?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1594736797933-d0501ba2fe65?w=400&h=260&fit=crop",
    ],
  },
  {
    id: "2",
    title: "Maison Rivière — Heritage",
    project: "Maison Rivière",
    style: ["Parisian", "Classic", "Ornate"],
    updated: "5d ago",
    liked: false,
    images: [
      "https://images.unsplash.com/photo-1586023492125-27b2c045efd7?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1567538096630-e0c55bd6374c?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1615529182904-14819c35db37?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1540518614846-7eded433c457?w=400&h=260&fit=crop",
    ],
  },
  {
    id: "3",
    title: "Aldgate — Industrial Raw",
    project: "The Aldgate Loft",
    style: ["Industrial", "Raw", "Brutalist"],
    updated: "1w ago",
    liked: true,
    images: [
      "https://images.unsplash.com/photo-1618221195710-dd6b41faaea6?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1574362848149-11496d93a7c7?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1565182999561-18d7dc61c393?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1560448204-e02f11c3d0e2?w=400&h=260&fit=crop",
    ],
  },
  {
    id: "4",
    title: "Villa Almería — Mediterranean",
    project: "Villa Almería",
    style: ["Mediterranean", "Earthy", "Textural"],
    updated: "2w ago",
    liked: false,
    images: [
      "https://images.unsplash.com/photo-1512917774080-9991f1c4c750?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1558618666-fcd25c85cd64?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1523217582562-09d0def993a6?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1564078516393-cf04bd966897?w=400&h=260&fit=crop",
    ],
  },
  {
    id: "5",
    title: "Cotswolds — Country Modern",
    project: "The Cotswolds Cottage",
    style: ["Rustic", "Warm", "Natural"],
    updated: "3d ago",
    liked: false,
    images: [
      "https://images.unsplash.com/photo-1568605114967-8130f3a36994?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1556020685-ae41abfc9365?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1513694203232-719a280e022f?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1484101403633-562f891dc89a?w=400&h=260&fit=crop",
    ],
  },
  {
    id: "6",
    title: "Shoreditch — Creative Studio",
    project: "Shoreditch Studio",
    style: ["Contemporary", "Eclectic", "Bold"],
    updated: "1m ago",
    liked: true,
    images: [
      "https://images.unsplash.com/photo-1497366216548-37526070297c?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1524758631624-e2822e304c36?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1519710164239-da123dc03ef4?w=400&h=260&fit=crop",
      "https://images.unsplash.com/photo-1486944936280-f152c4af4527?w=400&h=260&fit=crop",
    ],
  },
];

const ALL_STYLES = ["All", "Minimalist", "Warm", "Luxury", "Parisian", "Industrial", "Mediterranean", "Rustic", "Contemporary"];

const LAYOUT_VARIANTS = [
  // variant 0: big left + 2 right stacked
  (imgs: string[]) => (
    <div className="flex gap-1 h-48">
      <div className="flex-1 image-card rounded-l-lg" style={{ backgroundImage: `url(${imgs[0]})` }} />
      <div className="flex flex-col gap-1 w-32">
        <div className="flex-1 image-card rounded-tr-lg" style={{ backgroundImage: `url(${imgs[1]})` }} />
        <div className="flex-1 image-card rounded-br-lg" style={{ backgroundImage: `url(${imgs[2]})` }} />
      </div>
    </div>
  ),
  // variant 1: 2 + 2 equal
  (imgs: string[]) => (
    <div className="flex gap-1 h-48">
      <div className="flex flex-col gap-1 flex-1">
        <div className="flex-1 image-card rounded-tl-lg" style={{ backgroundImage: `url(${imgs[0]})` }} />
        <div className="flex-1 image-card rounded-bl-lg" style={{ backgroundImage: `url(${imgs[1]})` }} />
      </div>
      <div className="flex flex-col gap-1 flex-1">
        <div className="flex-1 image-card rounded-tr-lg" style={{ backgroundImage: `url(${imgs[2]})` }} />
        <div className="flex-1 image-card rounded-br-lg" style={{ backgroundImage: `url(${imgs[3]})` }} />
      </div>
    </div>
  ),
  // variant 2: big top + 3 bottom strip
  (imgs: string[]) => (
    <div className="flex flex-col gap-1 h-48">
      <div className="flex-1 image-card rounded-t-lg" style={{ backgroundImage: `url(${imgs[0]})` }} />
      <div className="flex gap-1 h-16">
        <div className="flex-1 image-card rounded-bl-lg" style={{ backgroundImage: `url(${imgs[1]})` }} />
        <div className="flex-1 image-card" style={{ backgroundImage: `url(${imgs[2]})` }} />
        <div className="flex-1 image-card rounded-br-lg" style={{ backgroundImage: `url(${imgs[3]})` }} />
      </div>
    </div>
  ),
];

interface MoodboardsProps {
  activeFilter?: string;
}

export default function Moodboards({ activeFilter = "All" }: MoodboardsProps) {
  const [styleFilter, setStyleFilter] = useState(activeFilter);
  const [likedMap, setLikedMap] = useState<Record<string, boolean>>(
    Object.fromEntries(BOARDS.map((b) => [b.id, b.liked]))
  );

  const filtered = styleFilter === "All"
    ? BOARDS
    : BOARDS.filter((b) => b.style.includes(styleFilter));

  return (
    <div data-cmp="Moodboards" className="flex-1 overflow-y-auto scrollbar-thin bg-background">
      <div className="max-w-[1100px] mx-auto px-6 py-5">
        {/* Toolbar */}
        <div className="flex items-center gap-3 mb-5">
          <div className="flex items-center gap-2 bg-surface border border-border rounded-lg px-3 py-2 flex-1 max-w-xs">
            <Search size={13} className="text-muted-foreground" />
            <input
              type="text"
              placeholder="Search moodboards…"
              className="bg-transparent text-[12px] font-sans text-foreground placeholder:text-muted-foreground flex-1 outline-none"
            />
          </div>
          <button className="flex items-center gap-1.5 px-3 py-2 border border-border rounded-lg text-[12px] font-sans text-muted-foreground hover:text-foreground hover:bg-surface transition-all">
            <Tag size={12} /> Style Tags
          </button>
          <button className="flex items-center gap-1.5 px-3 py-2 border border-border rounded-lg text-[12px] font-sans text-muted-foreground hover:text-foreground hover:bg-surface transition-all">
            <SlidersHorizontal size={12} /> Filter
          </button>
          <button className="flex items-center gap-1.5 bg-charcoal text-primary-foreground px-3 py-2 rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all ml-auto">
            <Plus size={13} /> New Board
          </button>
        </div>

        {/* Style filter pills */}
        <div className="flex items-center gap-2 flex-wrap mb-5">
          {ALL_STYLES.map((s) => (
            <button
              key={s}
              onClick={() => setStyleFilter(s)}
              className={`px-3 py-1 rounded-full text-[11px] font-sans font-medium transition-all ${
                styleFilter === s
                  ? "bg-charcoal text-primary-foreground"
                  : "bg-surface border border-border text-muted-foreground hover:text-foreground"
              }`}
            >
              {s}
            </button>
          ))}
        </div>

        {/* Grid — 3 columns */}
        <div className="flex gap-4 flex-wrap">
          {filtered.map((board, idx) => {
            const layoutFn = LAYOUT_VARIANTS[idx % LAYOUT_VARIANTS.length];
            return (
              <div
                key={board.id}
                className="w-[calc(33.333%-11px)] bg-surface border border-border rounded-xl shadow-custom overflow-hidden group cursor-pointer hover:border-muted-foreground/30 transition-all"
              >
                {/* Image mosaic */}
                <div className="p-2">
                  {layoutFn(board.images)}
                </div>

                {/* Info */}
                <div className="px-4 pb-4">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <h3 className="font-serif text-[14px] font-semibold text-foreground truncate">
                        {board.title}
                      </h3>
                      <p className="text-muted-foreground text-[10px] font-sans mt-0.5 truncate">
                        {board.project} · Updated {board.updated}
                      </p>
                    </div>
                    <div className="flex items-center gap-1 shrink-0">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setLikedMap((prev) => ({ ...prev, [board.id]: !prev[board.id] }));
                        }}
                        className={`w-7 h-7 rounded-md flex items-center justify-center transition-all ${
                          likedMap[board.id]
                            ? "bg-terracotta-light text-terracotta"
                            : "bg-muted text-muted-foreground hover:bg-terracotta-light hover:text-terracotta"
                        }`}
                      >
                        <Heart size={12} fill={likedMap[board.id] ? "currentColor" : "none"} />
                      </button>
                      <button className="w-7 h-7 rounded-md flex items-center justify-center bg-muted text-muted-foreground hover:bg-accent hover:text-foreground transition-all">
                        <Share2 size={12} />
                      </button>
                    </div>
                  </div>

                  {/* Style tags */}
                  <div className="flex items-center gap-1.5 mt-2.5 flex-wrap">
                    {board.style.map((t) => (
                      <span key={t} className="tag-pill">{t}</span>
                    ))}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
        <div className="h-6" />
      </div>
    </div>
  );
}
