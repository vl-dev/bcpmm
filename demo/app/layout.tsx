import { Providers } from './providers';

export const metadata = {
  title: 'CBMM Demo',
  description: 'CBMM Demo Application',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}

