import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'

export default function InstancesLoadingSkeleton() {
  return (
    <div className='flex flex-col h-full'>
      {/* Header Skeleton */}
      <div className='flex items-center justify-between border-b bg-background px-6 py-4'>
        <div className='flex items-center gap-4'>
          <Skeleton className='h-6 w-50' />
          <Skeleton className='h-10 w-75' />
        </div>
        <div className='flex items-center gap-2'>
          <Skeleton className='h-9 w-25' />
          <Skeleton className='h-9 w-35' />
        </div>
      </div>

      {/* Stats Skeleton */}
      <div className='grid grid-cols-4 gap-4 px-6 py-4 border-b bg-muted/20'>
        {[...Array(4)].map((_, i) => (
          <Card key={i}>
            <CardContent className='p-4'>
              <div className='flex items-center justify-between'>
                <div className='space-y-2'>
                  <Skeleton className='h-3 w-15' />
                  <Skeleton className='h-8 w-10' />
                </div>
                <Skeleton className='h-10 w-10 rounded-full' />
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Content Skeleton */}
      <div className='flex-1 px-6 py-4'>
        <div className='space-y-4'>
          <Skeleton className='h-10 w-100' />
          <div className='flex items-center gap-3'>
            <Skeleton className='h-10 flex-1 max-w-md' />
            <Skeleton className='h-10 w-40' />
            <Skeleton className='h-10 w-40' />
          </div>
          <Card>
            <CardContent className='p-6'>
              <div className='space-y-4'>
                {[...Array(5)].map((_, i) => (
                  <Skeleton key={i} className='h-16 w-full' />
                ))}
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}
