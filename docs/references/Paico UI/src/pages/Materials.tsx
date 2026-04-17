import { useState } from "react";
import { Search, Filter, Plus, Package, CheckCircle, AlertCircle, Clock } from "lucide-react";

type Material = {
  id: string;
  name: string;
  category: string;
  supplier: string;
  finish: string;
  color: string;
  stock: "in-stock" | "low-stock" | "out-of-stock";
  lead: string;
  price: string;
  unit: string;
  projects: string[];
  image: string;
};

const MATERIALS: Material[] = [
  { id: "1", name: "Travertine Honed Slab", category: "Stone", supplier: "Marmi Italiani", finish: "Honed", color: "Warm Ivory", stock: "in-stock", lead: "3 weeks", price: "$185", unit: "/m²", projects: ["Kensington Penthouse", "Maison Rivière"], image: "https://images.unsplash.com/photo-1558618666-fcd25c85cd64?w=300&h=200&fit=crop" },
  { id: "2", name: "Smoked Oak Chevron", category: "Timber", supplier: "Nordic Woods", finish: "Oiled", color: "Smoked", stock: "low-stock", lead: "5 weeks", price: "$95", unit: "/m²", projects: ["Kensington Penthouse"], image: "https://images.unsplash.com/photo-1591088398332-8a7791972843?w=300&h=200&fit=crop" },
  { id: "3", name: "Bouclé Upholstery Fabric", category: "Textile", supplier: "Métaphores", finish: "N/A", color: "Oatmeal", stock: "in-stock", lead: "2 weeks", price: "$320", unit: "/m", projects: ["Maison Rivière", "Cotswolds Cottage"], image: "https://images.unsplash.com/photo-1555041469-a586c61ea9bc?w=300&h=200&fit=crop" },
  { id: "4", name: "Aged Brass Fixtures", category: "Metalwork", supplier: "Waterworks UK", finish: "Unlacquered", color: "Aged Brass", stock: "in-stock", lead: "4 weeks", price: "POA", unit: "per set", projects: ["Kensington Penthouse", "Aldgate Loft"], image: "https://images.unsplash.com/photo-1594736797933-d0501ba2fe65?w=300&h=200&fit=crop" },
  { id: "5", name: "Zellige Wall Tiles", category: "Ceramic", supplier: "Moroccan Tiles Co.", finish: "Glazed", color: "Ivory Crackle", stock: "out-of-stock", lead: "8 weeks", price: "$145", unit: "/m²", projects: ["Villa Almería"], image: "https://images.unsplash.com/photo-1523217582562-09d0def993a6?w=300&h=200&fit=crop" },
  { id: "6", name: "Concrete Microtopping", category: "Surface", supplier: "Pandomo", finish: "Matt Sealed", color: "Warm Grey", stock: "in-stock", lead: "1 week", price: "$65", unit: "/m²", projects: ["Aldgate Loft", "Shoreditch Studio"], image: "https://images.unsplash.com/photo-1574362848149-11496d93a7c7?w=300&h=200&fit=crop" },
  { id: "7", name: "Natural Linen Curtain", category: "Textile", supplier: "Libeco", finish: "Raw", color: "Sand", stock: "in-stock", lead: "2 weeks", price: "$210", unit: "/m", projects: ["Cotswolds Cottage", "Kensington Penthouse"], image: "https://images.unsplash.com/photo-1540518614846-7eded433c457?w=300&h=200&fit=crop" },
  { id: "8", name: "Nero Marquina Marble", category: "Stone", supplier: "Salvatori", finish: "Polished", color: "Black & White", stock: "low-stock", lead: "6 weeks", price: "$340", unit: "/m²", projects: ["Maison Rivière"], image: "https://images.unsplash.com/photo-1616486338812-3dadae4b4ace?w=300&h=200&fit=crop" },
  { id: "9", name: "Rattan Woven Panel", category: "Craft", supplier: "Artisans de Bali", finish: "Natural", color: "Natural", stock: "in-stock", lead: "3 weeks", price: "$88", unit: "/m²", projects: ["Cotswolds Cottage"], image: "https://images.unsplash.com/photo-1565182999561-18d7dc61c393?w=300&h=200&fit=crop" },
];

const CATEGORIES = ["All", "Stone", "Timber", "Textile", "Metalwork", "Ceramic", "Surface", "Craft"];

const STOCK_MAP: Record<string, { label: string; icon: React.ComponentType<{ size?: number; className?: string }>; cls: string }> = {
  "in-stock": { label: "In Stock", icon: CheckCircle, cls: "text-olive" },
  "low-stock": { label: "Low Stock", icon: AlertCircle, cls: "text-gold" },
  "out-of-stock": { label: "Out of Stock", icon: Clock, cls: "text-terracotta" },
};

export default function Materials() {
  const [category, setCategory] = useState("All");
  const [selected, setSelected] = useState<string | null>(null);

  const filtered = category === "All" ? MATERIALS : MATERIALS.filter((m) => m.category === category);
  const mat = MATERIALS.find((m) => m.id === selected);

  return (
    <div data-cmp="Materials" className="flex-1 flex overflow-hidden">
      <div className="flex-1 overflow-y-auto scrollbar-thin bg-background">
        <div className="max-w-[960px] mx-auto px-6 py-5">
          {/* Toolbar */}
          <div className="flex items-center gap-3 mb-4">
            <div className="flex items-center gap-2 bg-surface border border-border rounded-lg px-3 py-2 flex-1 max-w-xs">
              <Search size={13} className="text-muted-foreground" />
              <input
                type="text"
                placeholder="Search materials…"
                className="bg-transparent text-[12px] font-sans text-foreground placeholder:text-muted-foreground flex-1 outline-none"
              />
            </div>
            <button className="flex items-center gap-1.5 px-3 py-2 border border-border rounded-lg text-[12px] font-sans text-muted-foreground hover:text-foreground hover:bg-surface transition-all">
              <Filter size={12} /> Filter
            </button>
            <button className="flex items-center gap-1.5 bg-charcoal text-primary-foreground px-3 py-2 rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all ml-auto">
              <Plus size={13} /> Add Material
            </button>
          </div>

          {/* Category pills */}
          <div className="flex items-center gap-2 flex-wrap mb-5">
            {CATEGORIES.map((c) => (
              <button
                key={c}
                onClick={() => setCategory(c)}
                className={`px-3 py-1 rounded-full text-[11px] font-sans font-medium transition-all ${
                  category === c
                    ? "bg-charcoal text-primary-foreground"
                    : "bg-surface border border-border text-muted-foreground hover:text-foreground"
                }`}
              >
                {c}
              </button>
            ))}
          </div>

          {/* Dense card grid */}
          <div className="flex flex-wrap gap-3">
            {filtered.map((m) => {
              const StockIcon = STOCK_MAP[m.stock].icon;
              return (
                <button
                  key={m.id}
                  onClick={() => setSelected(selected === m.id ? null : m.id)}
                  className={`w-[calc(33.333%-8px)] bg-surface border rounded-xl shadow-custom overflow-hidden text-left group hover:border-muted-foreground/30 transition-all ${
                    selected === m.id ? "border-charcoal" : "border-border"
                  }`}
                >
                  {/* Texture image */}
                  <div
                    className="h-32 image-card border-b border-border"
                    style={{ backgroundImage: `url(${m.image})` }}
                  />
                  <div className="p-3">
                    <div className="flex items-start justify-between gap-2 mb-1">
                      <div className="min-w-0">
                        <p className="text-[13px] font-sans font-semibold text-foreground truncate">
                          {m.name}
                        </p>
                        <p className="text-muted-foreground text-[10px] font-sans truncate">
                          {m.supplier} · {m.finish}
                        </p>
                      </div>
                      <span className="tag-pill shrink-0">{m.category}</span>
                    </div>

                    <div className="flex items-center justify-between mt-2">
                      <div className={`flex items-center gap-1 text-[10px] font-sans font-medium ${STOCK_MAP[m.stock].cls}`}>
                        <StockIcon size={10} />
                        {STOCK_MAP[m.stock].label}
                      </div>
                      <span className="text-[12px] font-sans font-semibold text-foreground">
                        {m.price}
                        <span className="text-muted-foreground text-[10px] font-normal">{m.unit}</span>
                      </span>
                    </div>

                    <div className="mt-2 border-t border-border pt-2 flex items-center justify-between">
                      <div className="flex items-center gap-1">
                        <Package size={9} className="text-muted-foreground" />
                        <span className="text-muted-foreground text-[10px] font-sans">
                          Lead: {m.lead}
                        </span>
                      </div>
                      <span className="text-muted-foreground text-[10px] font-sans">
                        {m.projects.length} project{m.projects.length !== 1 ? "s" : ""}
                      </span>
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
          <div className="h-6" />
        </div>
      </div>

      {/* Detail panel */}
      {mat && (
        <div className="w-72 shrink-0 border-l border-border bg-surface overflow-y-auto scrollbar-thin">
          <div className="sticky top-0 bg-surface border-b border-border px-4 py-3.5 flex items-center justify-between z-10">
            <h3 className="font-serif text-[14px] font-semibold text-foreground truncate">
              {mat.name}
            </h3>
            <button
              onClick={() => setSelected(null)}
              className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent transition-all text-[14px]"
            >
              ✕
            </button>
          </div>
          <div
            className="h-40 image-card border-b border-border"
            style={{ backgroundImage: `url(${mat.image})` }}
          />
          <div className="px-4 py-4">
            <div className="flex flex-col gap-2.5">
              {[
                { l: "Category", v: mat.category },
                { l: "Supplier", v: mat.supplier },
                { l: "Finish", v: mat.finish },
                { l: "Colour", v: mat.color },
                { l: "Lead Time", v: mat.lead },
                { l: "Unit Price", v: `${mat.price}${mat.unit}` },
              ].map((r) => (
                <div key={r.l} className="flex items-center justify-between">
                  <span className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide">
                    {r.l}
                  </span>
                  <span className="text-foreground text-[12px] font-sans font-medium">
                    {r.v}
                  </span>
                </div>
              ))}
            </div>

            <div className="mt-4">
              <p className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide mb-2">Applied to Projects</p>
              <div className="flex flex-col gap-1.5">
                {mat.projects.map((p) => (
                  <div key={p} className="flex items-center gap-2 px-2 py-1.5 bg-muted rounded-md">
                    <div className="w-1.5 h-1.5 rounded-full bg-olive" />
                    <span className="text-[12px] font-sans text-foreground">{p}</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="mt-5 flex flex-col gap-2">
              <button className="w-full py-2 bg-charcoal text-primary-foreground rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all">
                Request Sample
              </button>
              <button className="w-full py-2 border border-border text-foreground rounded-lg text-[12px] font-sans hover:bg-muted transition-all">
                Add to Project
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
