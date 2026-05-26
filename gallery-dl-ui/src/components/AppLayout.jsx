import { NavLink, Outlet } from 'react-router-dom';
import { Download, LayoutGrid, Image, Video, HardDrive, Users, Settings } from 'lucide-react';
import UrlSubmitForm from './UrlSubmitForm';
import './AppLayout.css';

export default function AppLayout() {
  return (
    <div className="app-layout">
      <aside className="sidebar">
        <div className="sidebar-logo">
          <HardDrive size={22} />
          <h1>Gallery DL</h1>
        </div>
        <nav className="sidebar-nav">
          <NavLink to="/" end>
            <Download size={18} />
            Downloads
          </NavLink>
          <NavLink to="/galleries">
            <LayoutGrid size={18} />
            Galleries
          </NavLink>
          <NavLink to="/people">
            <Users size={18} />
            People
          </NavLink>
          <NavLink to="/images">
            <Image size={18} />
            Images
          </NavLink>
          <NavLink to="/videos">
            <Video size={18} />
            Videos
          </NavLink>
          <div className="sidebar-spacer" />
          <NavLink to="/admin">
            <Settings size={18} />
            Admin
          </NavLink>
        </nav>
      </aside>
      <div className="main-content">
        <div className="topbar">
          <UrlSubmitForm />
        </div>
        <div className="page-content">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
