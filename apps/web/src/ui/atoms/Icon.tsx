const paths = {
  bolt: "M13 2 3 14h7l-1 8 12-14h-8l1-6Z",
  users:
    "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M22 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75",
  calendar:
    "M8 2v4 M16 2v4 M3 10h18 M5 4h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z",
  flag: "M4 22V3 M4 3h14l-3 4 3 4H4",
  globe:
    "M2 12h20 M12 2c6 6 6 14 0 20 M12 2c-6 6-6 14 0 20 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z",
  settings: "M4 6h16 M4 12h16 M4 18h16 M8 3v6 M16 9v6 M10 15v6",
  arrow: "m9 4 8 8-8 8",
};
export type IconName = keyof typeof paths;
export function Icon({ name }: { name: IconName }) {
  return (
    <svg
      aria-hidden="true"
      class="icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d={paths[name]} />
    </svg>
  );
}
