import type { Page } from './types';

interface NavbarProps {
  page: Page;
  setPage: (page: Page) => void;
}

const pages: { key: Page; label: string }[] = [
  { key: 'dashboard', label: 'Dashboard' },
  { key: 'chain', label: 'Chain' },
  { key: 'accounts', label: 'Accounts' },
  { key: 'whitepaper', label: 'Whitepaper' },
];

export default function Navbar({ page, setPage }: NavbarProps) {
  return (
    <nav className="navbar">
      {pages.map(({ key, label }) => (
        <button
          key={key}
          onClick={() => setPage(key)}
          className={`navbar-btn ${page === key ? 'navbar-btn--active' : 'navbar-btn--inactive'}`}
        >
          {label}
        </button>
      ))}
    </nav>
  );
}
