import { BrowserRouter, Routes, Route } from 'react-router-dom';
import AppLayout from './components/AppLayout';
import DownloadsPage from './pages/DownloadsPage';
import GalleriesPage from './pages/GalleriesPage';
import GalleryDetailPage from './pages/GalleryDetailPage';
import ImagesPage from './pages/ImagesPage';
import VideosPage from './pages/VideosPage';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<DownloadsPage />} />
          <Route path="/galleries" element={<GalleriesPage />} />
          <Route path="/galleries/:id" element={<GalleryDetailPage />} />
          <Route path="/images" element={<ImagesPage />} />
          <Route path="/videos" element={<VideosPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
