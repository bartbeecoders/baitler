import { Link } from 'react-router-dom';
import { ArrowRight } from 'lucide-react';

import { featureItems } from '@/config/navigation';
import { Badge } from '@/components/ui/badge';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { SystemStatus } from './SystemStatus';

/** The base portal landing page — the hub linking to every workspace. */
export function Dashboard() {
  return (
    <div className="flex flex-col gap-10">
      <section>
        <p className="text-sm font-medium text-primary-strong">Base portal</p>
        <h1 className="mt-1 text-3xl font-bold tracking-tight sm:text-4xl">Welcome to Baitler</h1>
        <p className="mt-2 max-w-2xl text-muted-foreground">
          Your AI butler for files, ideas, and data. Everything comes together here — pick a
          workspace to get started.
        </p>
      </section>

      <section aria-labelledby="features-heading">
        <h2 id="features-heading" className="sr-only">
          Features
        </h2>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {featureItems.map((item) => (
            <Link
              key={item.path}
              to={item.path}
              className="group rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
            >
              <Card className="h-full transition-colors group-hover:border-primary/50 group-focus-visible:border-primary/50">
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <span className="grid h-10 w-10 place-items-center rounded-md bg-accent text-accent-foreground">
                      <item.icon className="h-5 w-5" aria-hidden="true" />
                    </span>
                    {item.phase !== null && <Badge variant="muted">Phase {item.phase}</Badge>}
                  </div>
                  <CardTitle className="mt-3 flex items-center gap-1">
                    {item.label}
                    <ArrowRight
                      className="h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
                      aria-hidden="true"
                    />
                  </CardTitle>
                  <CardDescription>{item.description}</CardDescription>
                </CardHeader>
              </Card>
            </Link>
          ))}
        </div>
      </section>

      <SystemStatus />
    </div>
  );
}
