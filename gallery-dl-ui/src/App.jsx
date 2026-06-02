import { BrowserRouter, Routes, Route } from 'react-router-dom';
import AppLayout from './components/AppLayout';
import DownloadsPage from './pages/DownloadsPage';
import RequestDetailPage from './pages/RequestDetailPage';
import GalleriesPage from './pages/GalleriesPage';
import GalleryDetailPage from './pages/GalleryDetailPage';
import PeoplePage from './pages/PeoplePage';
import PersonDetailPage from './pages/PersonDetailPage';
import ImagesPage from './pages/ImagesPage';
import VideosPage from './pages/VideosPage';
import AdminPage from './pages/AdminPage';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<DownloadsPage />} />
          <Route path="/downloads/:id" element={<RequestDetailPage />} />
          <Route path="/galleries" element={<GalleriesPage />} />
          <Route path="/galleries/:id" element={<GalleryDetailPage />} />
          <Route path="/people" element={<PeoplePage />} />
          <Route path="/people/:id" element={<PersonDetailPage />} />
          <Route path="/images" element={<ImagesPage />} />
          <Route path="/videos" element={<VideosPage />} />
          <Route path="/admin" element={<AdminPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
