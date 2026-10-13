// Small inline icon set (stroke icons, 20px grid) so nothing loads remotely.
const paths: Record<string, string> = {
  plus: "M10 4v12M4 10h12",
  back: "M12.5 4.5 7 10l5.5 5.5",
  search: "M9 15a6 6 0 1 0 0-12 6 6 0 0 0 0 12Zm4.5-1.5L17 17",
  post: "M5 3.5h10v13H5zM7.5 7h5M7.5 10h5M7.5 13h3",
  page: "M6 2.5h5.5L15 6v11.5H6zM11.5 2.5V6H15",
  type: "M4 5h12M4 10h12M4 15h8",
  undo: "M7 8H13a3.5 3.5 0 0 1 0 7H9M7 8l3-3M7 8l3 3",
  redo: "M13 8H7a3.5 3.5 0 0 0 0 7h4M13 8l-3-3M13 8l-3 3",
  settings: "M3.5 4.5h13v11h-13zM12 4.5v11",
  desktop: "M3 4h14v9H3zM7.5 16.5h5M10 13v3.5",
  tablet: "M5.5 2.5h9v15h-9zM9 15h2",
  mobile: "M7 2.5h6v15H7zM9.5 15h1",
  external: "M11 3.5h5.5V9M16.5 3.5 9 11M14 12v4.5H3.5V6H8",
  trash: "M4 6h12M8 6V4h4v2M5.5 6l.8 10.5h7.4L14.5 6",
  more: "M5 10h.01M10 10h.01M15 10h.01",
  globe: "M10 17a7 7 0 1 0 0-14 7 7 0 0 0 0 14ZM3 10h14M10 3c2 2 2.8 4.5 2.8 7S12 15 10 17c-2-2-2.8-4.5-2.8-7S8 5 10 3Z",
  lock: "M5.5 9h9v7.5h-9zM7.5 9V6.5a2.5 2.5 0 0 1 5 0V9",
  check: "M4.5 10.5 8 14l7.5-8",
  refresh: "M15.5 8A6 6 0 0 0 4.6 6.5M4.5 3.5v3h3M4.5 12a6 6 0 0 0 10.9 1.5M15.5 16.5v-3h-3",
};

export function Icon({ name, size = 18, title }: { name: keyof typeof paths | string; size?: number; title?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
    >
      {title ? <title>{title}</title> : null}
      <path d={paths[name] ?? paths.post} />
    </svg>
  );
}
