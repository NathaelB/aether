import { Outlet } from '@tanstack/react-router'

export function OnboardingLayout() {
  return (
    <div className='flex min-h-screen w-full flex-col p-6'>
      <div className='mx-auto w-full max-w-5xl flex-1'>
        <Outlet />
      </div>
    </div>
  )
}
