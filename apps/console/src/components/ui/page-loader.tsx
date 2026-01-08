import { Spinner } from './spinner'
import { cn } from '@/lib/utils'

interface PageLoaderProps {
  className?: string
}

export function PageLoader({ className }: PageLoaderProps) {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center h-full w-full',
        className
      )}
    >
      <Spinner className='text-gray-500' />
    </div>
  )
}
