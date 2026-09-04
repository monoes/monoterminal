/**
 * Small stroke-icon set for the app shell — replaces emoji glyphs (🖥📁▸☰)
 * which render inconsistently across OS/font and read as placeholder art,
 * not a designed icon system. One consistent geometry (24x24 viewBox,
 * 1.75px stroke, round caps) instead.
 */
import type { SVGProps } from 'react';

function Icon({ children, ...props }: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {children}
    </svg>
  );
}

export function IconMonitor(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <rect x="2.5" y="4" width="19" height="13" rx="1.5" />
      <line x1="8" y1="20.5" x2="16" y2="20.5" />
      <line x1="12" y1="17" x2="12" y2="20.5" />
    </Icon>
  );
}

export function IconFolder(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <path d="M3 6.5a1.5 1.5 0 0 1 1.5-1.5h4.6a1.5 1.5 0 0 1 1.2.6l1.1 1.4h9.1A1.5 1.5 0 0 1 22 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-17A1.5 1.5 0 0 1 2 17.5v-11Z" />
    </Icon>
  );
}

export function IconPrompt(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <path d="M6 8.5 10.5 12 6 15.5" />
      <line x1="12.5" y1="16.5" x2="18" y2="16.5" />
    </Icon>
  );
}

export function IconPlus(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </Icon>
  );
}

export function IconClose(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <line x1="6" y1="6" x2="18" y2="18" />
      <line x1="18" y1="6" x2="6" y2="18" />
    </Icon>
  );
}

export function IconMenu(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <line x1="4" y1="7" x2="20" y2="7" />
      <line x1="4" y1="12" x2="20" y2="12" />
      <line x1="4" y1="17" x2="20" y2="17" />
    </Icon>
  );
}

export function IconPanel(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <rect x="2.5" y="4" width="19" height="16" rx="1.5" />
      <line x1="15" y1="4" x2="15" y2="20" />
    </Icon>
  );
}

export function IconSplitRight(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <rect x="3" y="4" width="18" height="16" rx="1.5" />
      <line x1="12" y1="4" x2="12" y2="20" />
    </Icon>
  );
}

export function IconSplitDown(props: SVGProps<SVGSVGElement>) {
  return (
    <Icon {...props}>
      <rect x="3" y="4" width="18" height="16" rx="1.5" />
      <line x1="3" y1="12" x2="21" y2="12" />
    </Icon>
  );
}
