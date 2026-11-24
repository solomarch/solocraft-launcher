const PATHS: Record<string, string> = {
  play: "M7 4l13 8-13 8z",
  plus: "M12 5v14M5 12h14",
  x: "M6 6l12 12M18 6L6 18",
  check: "M5 12l5 5 9-10",
  search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
  sliders: "M4 7h8M16 7h4M4 17h4M12 17h8M14 5a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM10 15a2 2 0 1 0 0 4 2 2 0 0 0 0-4z",
  cube: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5",
  user: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0",
  bolt: "M13 3L5 14h6l-1 7 8-11h-6z",
  stop: "M6 6h12v12H6z",
  terminal: "M4 5h16v14H4zM7 9l3 3-3 3M12 15h5",
  copy: "M9 9h11v11H9zM5 15V5h10",
  file: "M6 3h8l4 4v14H6zM14 3v4h4M9 12h6M9 16h6",
  coffee: "M5 8h12v6a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4zM17 9h2a2 2 0 0 1 0 4h-2M8 3v2M12 3v2",
  home: "M4 11l8-7 8 7v9H4z",
  edit: "M4 20h4l10-10-4-4L4 16zM13 7l4 4",
  grid: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
  download: "M12 4v12M7 11l5 5 5-5M5 20h14",
  puzzle: "M10 4a2 2 0 1 1 4 0v2h4v4h-2a2 2 0 1 0 0 4h2v4h-4v-2a2 2 0 1 0-4 0v2H6v-4H4a2 2 0 1 1 0-4h2V6h4z",
  refresh: "M20 11a8 8 0 1 0-2 6M20 4v7h-7",
};

export function Icon({ name }: { name: keyof typeof PATHS | string }) {
  return (
    <svg className="ic" viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name] ?? ""} />
    </svg>
  );
}
